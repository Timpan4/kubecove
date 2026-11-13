use super::*;
use serde_json::json;

fn row(
    kind: &str,
    name: &str,
    state: HealthAssessmentState,
    owner: Option<&str>,
) -> ResourceSummary {
    serde_json::from_value(json!({
        "kind": kind, "name": name, "cluster": "dev", "namespace": "apps", "age": "1d",
        "ownerRef": owner,
        "healthAssessment": { "state": state, "completeness": "complete", "winningSources": ["kubernetes"], "reasons": [], "evidence": [] }
    })).unwrap()
}
#[test]
fn folds_an_owner_chain_and_uses_the_winning_pod_cause() {
    let mut pod = row("Pod", "pod", HealthAssessmentState::Degraded, Some("rs"));
    pod.restarts = Some(12);
    let rows = [
        row("Deployment", "api", HealthAssessmentState::Degraded, None),
        row(
            "ReplicaSet",
            "rs",
            HealthAssessmentState::Degraded,
            Some("api"),
        ),
        pod,
    ];
    let model = build(&rows.iter().collect::<Vec<_>>());
    assert_eq!(model.problems.len(), 1);
    assert_eq!(model.problem_resources, 3);
    assert_eq!(model.problems[0].root, 0);
    assert_eq!(model.problems[0].cause, 2);
    assert_eq!(model.problems[0].related, vec![1, 2]);
}
#[test]
fn never_merges_contexts_namespaces_ambiguous_or_cyclic_owners() {
    let mut rows = vec![
        row("Deployment", "api", HealthAssessmentState::Degraded, None),
        row("Pod", "pod", HealthAssessmentState::Degraded, Some("api")),
    ];
    let mut other = rows.clone();
    for row in &mut other {
        row.cluster = "prod".into();
    }
    rows.extend(other);
    let model = build(&rows.iter().collect::<Vec<_>>());
    assert_eq!(model.problems.len(), 2);
    assert!(
        model
            .problems
            .iter()
            .all(|problem| problem.related.len() == 1)
    );
    let rows = [
        row("Deployment", "same", HealthAssessmentState::Degraded, None),
        row("StatefulSet", "same", HealthAssessmentState::Degraded, None),
        row(
            "Pod",
            "ambiguous",
            HealthAssessmentState::Degraded,
            Some("same"),
        ),
        row(
            "Pod",
            "orphan",
            HealthAssessmentState::Degraded,
            Some("missing"),
        ),
        row(
            "ReplicaSet",
            "cycle-a",
            HealthAssessmentState::Degraded,
            Some("cycle-b"),
        ),
        row(
            "ReplicaSet",
            "cycle-b",
            HealthAssessmentState::Degraded,
            Some("cycle-a"),
        ),
    ];
    assert_eq!(
        build(&rows.iter().collect::<Vec<_>>()).problems.len(),
        rows.len()
    );
}
#[test]
fn healthy_intermediates_do_not_hide_findings_or_replace_a_degraded_cause() {
    let rows = [
        row("Deployment", "api", HealthAssessmentState::Degraded, None),
        row(
            "ReplicaSet",
            "rs",
            HealthAssessmentState::Healthy,
            Some("api"),
        ),
        row(
            "Pod",
            "pod",
            HealthAssessmentState::NeedsAttention,
            Some("rs"),
        ),
    ];
    let model = build(&rows.iter().collect::<Vec<_>>());
    assert_eq!(model.problems.len(), 1);
    assert_eq!(model.problems[0].cause, 0);
    assert_eq!(model.problems[0].related, vec![2]);
}
#[test]
fn same_named_kinds_have_a_stable_order() {
    let rows = [
        row("Ingress", "api", HealthAssessmentState::Degraded, None),
        row("Deployment", "api", HealthAssessmentState::Degraded, None),
    ];
    let references = rows.iter().collect::<Vec<_>>();
    let model = build(&references);
    assert_eq!(references[model.problems[0].root].kind, "Deployment");
    assert_eq!(references[model.problems[1].root].kind, "Ingress");
}
#[test]
fn historical_restarts_are_not_current_findings() {
    let mut recovered = row("Pod", "recovered", HealthAssessmentState::Healthy, None);
    recovered.restarts = Some(12);
    let rows = [
        recovered,
        row(
            "ConfigMap",
            "config",
            HealthAssessmentState::NotEvaluated,
            None,
        ),
        row("Pod", "unknown", HealthAssessmentState::Unknown, None),
    ];
    let model = build(&rows.iter().collect::<Vec<_>>());
    assert!(model.problems.is_empty());
    assert_eq!(model.namespaces[0].total, 3);
    assert_eq!(model.namespaces[0].degraded, 0);
}
