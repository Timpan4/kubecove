use super::*;
use kubecove_lib::models::{HealthAssessmentState, WatchResourceKey};
use serde_json::json;

// Failure modes: a named update affects another namespace; health-only updates
// leave the summary stale; ownership changes keep old search/group fields;
// deleting and inserting rows at equal cardinality preserves obsolete indices.
fn row(name: &str, namespace: &str) -> ResourceSummary {
    serde_json::from_value(json!({
        "name":name, "kind":"Pod", "cluster":"test", "namespace":namespace, "age":"1d"
    }))
    .unwrap()
}
fn key() -> WatchResourceKey {
    serde_json::from_value(json!({"resourceKind":{"kind":"Pod"}, "namespace":null})).unwrap()
}
#[test]
fn named_patch_updates_health_without_crossing_namespaces() {
    let unchanged = row("same", "b");
    let expected = unchanged.health_assessment.state;
    let mut rows = PreparedRows::new(vec![row("same", "a"), unchanged]);
    let mut update = row("same", "a");
    update.health_assessment.state = HealthAssessmentState::Degraded;
    rows.patch(
        &key(),
        vec![update],
        Some([(Some("a".into()), "same".into())].into_iter().collect()),
    );
    let mut visible = Visible::default();
    visible.refilter(Rc::new(rows), &["Pod".into()], "");
    assert_eq!(visible.len(), 2);
    assert_eq!(visible.health().degraded, 1);
    assert_eq!(
        visible
            .iter()
            .find(|row| row.namespace.as_deref() == Some("b"))
            .unwrap()
            .health_assessment
            .state,
        expected
    );
}

// A full read must not patch unannounced rows when notices contain duplicates
// or arrive in another order. A complete ordered notice set updates every row.
#[test]
fn named_batch_requires_complete_coverage_before_replacing_every_row() {
    let original = vec![row("a", "ns"), row("b", "ns"), row("c", "ns")];
    let mut rows = PreparedRows::new(original.clone());
    let mut changed = original.clone();
    for row in &mut changed {
        row.health_assessment.state = HealthAssessmentState::Degraded;
    }
    let notices = |names: &[&str]| {
        names
            .iter()
            .map(|name| (Some("ns".into()), (*name).into()))
            .collect()
    };
    rows.patch(&key(), changed, Some(notices(&["a", "b", "c"])));
    assert!(
        rows.iter()
            .all(|row| row.health_assessment.state == HealthAssessmentState::Degraded)
    );
    rows.patch(&key(), original.clone(), Some(notices(&["b", "a", "b"])));
    for row in rows.iter() {
        let expected = if row.name == "c" {
            HealthAssessmentState::Degraded
        } else {
            original[0].health_assessment.state
        };
        assert_eq!(row.health_assessment.state, expected, "{}", row.name);
    }
}
#[test]
fn replacement_patch_rebuilds_search_and_group_fields() {
    let mut rows = PreparedRows::new(vec![row("removed", "a"), row("same", "b")]);
    let mut update = row("same", "b");
    update.argo_app = Some("new-owner".into());
    rows.patch(&key(), vec![update, row("inserted", "a")], None);
    let rows = Rc::new(rows);
    let mut visible = Visible::default();
    visible.refilter(rows.clone(), &["Pod".into()], "new-owner");
    assert_eq!(visible.len(), 1);
    assert_eq!(visible.iter().next().unwrap().name, "same");
    assert!(visible.grouped_by_gitops);
    visible.refilter(rows.clone(), &["Pod".into()], "removed");
    assert!(visible.is_empty());
    visible.refilter(rows, &["Pod".into()], "inserted");
    assert_eq!(visible.len(), 1);
    assert_eq!(
        visible.iter().next().unwrap().namespace.as_deref(),
        Some("a")
    );
}
