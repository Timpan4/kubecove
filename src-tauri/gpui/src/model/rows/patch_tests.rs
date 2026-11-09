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

#[test]
fn ordered_patch_rebuilds_owner_search_without_changing_row_identity() {
    let original = row("same", "ns");
    let mut rows = PreparedRows::new(vec![original.clone()]);
    let mut changed = original;
    changed.argo_app = Some("new-owner".into());
    rows.patch(&key(), vec![changed], None);
    let mut visible = Visible::default();
    visible.refilter(Rc::new(rows), &["Pod".into()], "new-owner");
    assert_eq!(visible.len(), 1);
    assert!(visible.grouped_by_gitops);
}

// Inherited owners live in the rows, but watch rows carry the backend's owners.
// Failure modes: an unchanged watch read drops a Pod's inherited owner or counts
// as a key change; a parent's new owner does not reach its Pods; rows taken back
// for a new read keep inherited owners as if the backend sent them.
#[test]
fn watch_patches_keep_inherited_owners_in_step_with_their_parents() {
    let parent = |app: &str| -> ResourceSummary {
        serde_json::from_value(json!({
            "name":"rs", "kind":"ReplicaSet", "cluster":"test", "namespace":"ns", "age":"1d",
            "gitOpsOwner":{"provider":"argo", "kind":"Application", "name":app,
                "confidence":"label", "provenance":"tracking-id", "partial":false}
        }))
        .unwrap()
    };
    let mut pod = row("pod", "ns");
    pod.owner_ref = Some("rs".into());
    let owner = |rows: &PreparedRows| {
        let pod = rows.iter().find(|row| row.kind == "Pod").unwrap();
        pod.git_ops_owner.as_ref().map(|owner| owner.name.clone())
    };
    let mut rows = PreparedRows::with_inherited_owners(vec![parent("app"), pod.clone()]);
    assert_eq!(owner(&rows), Some("app".into()));

    let identity = rows.identity();
    rows.patch(&key(), vec![pod.clone()], None);
    assert_eq!(owner(&rows), Some("app".into()));
    assert!(
        rows.identity() == identity,
        "unchanged read rebuilt the rows"
    );

    let replica_sets: WatchResourceKey =
        serde_json::from_value(json!({"resourceKind":{"kind":"ReplicaSet"}, "namespace":null}))
            .unwrap();
    rows.patch(&replica_sets, vec![parent("other")], None);
    assert_eq!(owner(&rows), Some("other".into()));

    let backend = rows.backend_rows();
    let pod = backend.iter().find(|row| row.kind == "Pod").unwrap();
    assert_eq!(pod.git_ops_owner, None);
}

// A named read holds only the rows its notices name. Failure modes: a named row
// missing from the read lingers; a named row new in the read is not added; a
// row of another kind with the same name is replaced; an in-place update
// reports another row as changed or rebuilds the rows.
#[test]
fn named_reads_replace_add_and_delete_only_their_rows() {
    let service = |name: &str| {
        let mut row = row(name, "ns");
        row.kind = "Service".into();
        row
    };
    let degraded = |name: &str| {
        let mut row = row(name, "ns");
        row.health_assessment.state = HealthAssessmentState::Degraded;
        row
    };
    let names = |names: &[&str]| {
        names
            .iter()
            .map(|name| (Some("ns".into()), (*name).into()))
            .collect()
    };
    let mut rows = PreparedRows::new(vec![
        row("a", "ns"),
        row("b", "ns"),
        row("c", "ns"),
        service("a"),
    ]);
    let patched = rows.patch(
        &key(),
        vec![degraded("a"), row("d", "ns")],
        Some(names(&["a", "b", "d"])),
    );
    assert!(
        patched.changed.is_none(),
        "an add or delete must prepare the rows again"
    );
    let state = |rows: &PreparedRows, kind: &str, name: &str| {
        rows.iter()
            .find(|row| row.kind == kind && row.name == name)
            .map(|row| row.health_assessment.state)
    };
    assert_eq!(
        state(&rows, "Pod", "a"),
        Some(HealthAssessmentState::Degraded)
    );
    assert_eq!(state(&rows, "Pod", "b"), None);
    assert!(state(&rows, "Pod", "d").is_some());
    assert_ne!(
        state(&rows, "Service", "a"),
        Some(HealthAssessmentState::Degraded)
    );

    let identity = rows.identity();
    let patched = rows.patch(&key(), vec![degraded("c")], Some(names(&["c"])));
    let c = rows
        .iter()
        .position(|row| row.kind == "Pod" && row.name == "c");
    assert_eq!(patched.changed, Some(vec![c.unwrap()]));
    assert!(rows.identity() == identity);
    assert_eq!(
        state(&rows, "Pod", "c"),
        Some(HealthAssessmentState::Degraded)
    );
    assert_eq!(
        state(&rows, "Pod", "a"),
        Some(HealthAssessmentState::Degraded)
    );
}
