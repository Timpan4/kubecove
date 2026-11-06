use super::*;
use serde_json::json;

// Preserve deduplication, dated/undated group order, expansion keys and times.
#[test]
fn projected_rows_keep_timeline_content() {
    let summary = serde_json::from_value(
        json!({"name":"pod","kind":"Pod","cluster":"test","namespace":"ns","age":"1d"}),
    )
    .unwrap();
    let events: Vec<ResourceEventSummary> = [Some("2026-11-01T13:00:00Z"), None, Some("2026-10-31T00:00:00Z"), None].into_iter().map(|time| {
        serde_json::from_value(json!({"eventType":"Warning","reason":"BackOff","message":"retry","count":2,"lastSeen":"1m","lastSeenAt":time,"source":"test"})).unwrap()
    }).collect();
    let model = Model::new(&summary, &[], &events);
    let expected = entries::incident_timeline(&summary, &[], &events);
    assert_eq!(model.items, expected);
    for (actual, expected) in model.groups.iter().zip(entries::group_by_day(&expected)) {
        assert_eq!(actual.key, expected.key);
        assert_eq!(actual.label, expected.label);
        assert_eq!(actual.rows.len(), expected.items.len());
        for (row, item) in actual.rows.iter().zip(expected.items) {
            assert_eq!(model.items[row.item], *item);
            assert_eq!(row.key, item.key());
            assert_eq!(
                row.time,
                if actual.key.is_some() {
                    entries::timeline_time(item)
                } else {
                    String::new()
                }
            );
        }
    }
}
