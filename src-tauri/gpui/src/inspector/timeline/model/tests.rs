use super::*;
use serde_json::json;

fn pod() -> ResourceSummary {
    serde_json::from_value(
        json!({"name":"pod","kind":"Pod","cluster":"test","namespace":"ns","age":"1d"}),
    )
    .unwrap()
}

// Preserve deduplication, dated/undated group order, expansion keys and times.
#[test]
fn projected_rows_keep_timeline_content() {
    let summary = pod();
    let events: Vec<ResourceEventSummary> = [Some("2026-11-01T13:00:00Z"), None, Some("2026-10-31T00:00:00Z"), None].into_iter().map(|time| {
        serde_json::from_value(json!({"eventType":"Warning","reason":"BackOff","message":"retry","count":2,"lastSeen":"1m","lastSeenAt":time,"source":"test"})).unwrap()
    }).collect();
    let timezone = TimestampTimezone::Local;
    let model = Model::new(&summary, &[], &events, timezone);
    let expected = entries::incident_timeline(&summary, &[], &events);
    assert_eq!(model.items, expected);
    for (actual, expected) in model
        .groups
        .iter()
        .zip(entries::group_by_day(&expected, timezone))
    {
        assert_eq!(actual.key, expected.key);
        assert_eq!(actual.label, expected.label);
        assert_eq!(actual.rows.len(), expected.items.len());
        for (row, item) in actual.rows.iter().zip(expected.items) {
            assert_eq!(model.items[row.item], *item);
            assert_eq!(row.key, item.key());
            assert_eq!(
                row.time,
                if actual.key.is_some() {
                    entries::timeline_time(item, timezone)
                } else {
                    String::new()
                }
            );
        }
    }
}

// Failure mode: day keys, labels and clock times use the machine's timezone
// while the Timestamp timezone setting is UTC, so entries near midnight land
// under the wrong day.
#[test]
fn utc_setting_groups_and_times_entries_in_utc() {
    let events: Vec<ResourceEventSummary> = [
        ("BackOff", "2026-10-31T23:30:00Z"),
        ("Failed", "2026-11-01T00:30:00Z"),
    ]
    .into_iter()
    .map(|(reason, time)| {
        serde_json::from_value(json!({"eventType":"Warning","reason":reason,"message":"retry","count":1,"lastSeen":"1m","lastSeenAt":time,"source":"test"})).unwrap()
    })
    .collect();
    let model = Model::new(&pod(), &[], &events, TimestampTimezone::Utc);
    let days: Vec<_> = model
        .groups
        .iter()
        .map(|group| {
            (
                group.key.as_deref(),
                group.label.as_str(),
                group
                    .rows
                    .iter()
                    .map(|row| row.time.as_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        days,
        [
            (Some("2026-10-31"), "31 Oct 2026", vec!["23:30:00"]),
            (Some("2026-11-01"), "1 Nov 2026", vec!["00:30:00"]),
        ]
    );
}

// Failure mode: Unicode lowercasing turns `İ` into two characters, so a match
// offset taken from the lowercased text cuts the resource name to `pu`.
#[test]
fn scheduling_title_reads_the_resource_after_non_ascii_text() {
    let event: ResourceEventSummary = serde_json::from_value(json!({"eventType":"Warning","reason":"FailedScheduling","message":"İ: Insufficient cpu","count":1,"lastSeen":"1m","lastSeenAt":null,"source":"test"})).unwrap();
    let items = entries::incident_timeline(&pod(), &[], &[event]);
    assert_eq!(
        entries::timeline_title(&items[0], "Pod"),
        "Scheduling blocked: insufficient cpu"
    );
}
