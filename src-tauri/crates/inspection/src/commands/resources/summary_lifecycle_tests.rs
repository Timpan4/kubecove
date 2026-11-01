use super::super::builtin_summary::builtin_summary;
use super::*;
use chrono::Utc;
use serde_json::{json, Value};

// Failure modes: deletion is hidden by Running/Ready; list projection drops waiting
// reasons or rollout evidence; readiness alone reports a rollout complete while
// old replicas still exist; stalled/paused workloads animate as normal progress.
fn pod(status: Value) -> Value {
    json!({"apiVersion":"v1", "kind":"Pod", "metadata":{"name":"pod", "namespace":"app"},
        "status":status})
}

#[test]
fn deleting_pods_are_terminating_even_when_running_and_ready() {
    let mut value =
        pod(json!({"phase":"Running", "conditions":[{"type":"Ready","status":"True"}]}));
    value["metadata"]["deletionTimestamp"] = json!("2026-10-30T00:00:00Z");
    let projected: PodProjection = serde_json::from_value(value.clone()).unwrap();
    let full: Pod = serde_json::from_value(value).unwrap();
    for pod in [full, projected.into_resource()] {
        let summary = builtin_summary("dev", &pod, Utc::now());
        assert_eq!(summary.status.as_deref(), Some("Terminating"));
        assert_eq!(summary.ready.as_deref(), Some("True"));
    }
}

#[test]
fn pod_startup_does_not_hide_image_pull_or_crash_loop_failures() {
    for (phase, ready, reason, expected) in [
        ("Pending", "False", "ContainerCreating", "Starting"),
        ("Pending", "False", "ImagePullBackOff", "ImagePullBackOff"),
        ("Running", "False", "CrashLoopBackOff", "CrashLoopBackOff"),
        ("Running", "True", "", "Running"),
        ("Succeeded", "False", "", "Succeeded"),
    ] {
        let value = pod(
            json!({"phase":phase, "conditions":[{"type":"Ready","status":ready}],
            "containerStatuses":[{"name":"app", "image":"app", "imageID":"id", "ready":false,
                "restartCount":0, "state":{"waiting":{"reason":reason}}}]}),
        );
        let projected: PodProjection = serde_json::from_value(value.clone()).unwrap();
        let full: Pod = serde_json::from_value(value).unwrap();
        for pod in [full, projected.into_resource()] {
            let summary = builtin_summary("dev", &pod, Utc::now());
            assert_eq!(
                summary.status.as_deref(),
                Some(expected),
                "{phase}/{reason}"
            );
        }
    }
}

#[test]
fn deployment_rollout_requires_observed_generation_and_all_replicas_updated() {
    for (observed, updated, replicas, paused, condition, expected) in [
        (1, 1, 1, false, json!([]), "Rolling out"),
        (2, 0, 1, false, json!([]), "Rolling out"),
        (2, 1, 2, false, json!([]), "Rolling out"),
        (2, 1, 1, false, json!([]), "Available: 1"),
        (1, 0, 1, true, json!([]), "Paused"),
        (
            2,
            0,
            1,
            false,
            json!([{"type":"Progressing","status":"False","reason":"ProgressDeadlineExceeded"}]),
            "Rollout failed",
        ),
    ] {
        let value = json!({"apiVersion":"apps/v1", "kind":"Deployment", "metadata":{"name":"api", "generation":2},
            "spec":{"replicas":1,"paused":paused,"selector":{},"template":{}},
            "status":{"observedGeneration":observed,"updatedReplicas":updated,"replicas":replicas,
                "readyReplicas":1,"availableReplicas":1,"conditions":condition}});
        let projected: DeploymentProjection = serde_json::from_value(value.clone()).unwrap();
        let full: Deployment = serde_json::from_value(value).unwrap();
        for deployment in [full, projected.into_resource()] {
            let summary = builtin_summary("dev", &deployment, Utc::now());
            assert_eq!(summary.status.as_deref(), Some(expected));
        }
    }
}

#[test]
fn omitted_zero_replica_counts_do_not_hide_a_rollout() {
    let value = json!({"metadata":{"name":"api", "generation":2},
        "spec":{"replicas":1,"selector":{},"template":{}},
        "status":{"observedGeneration":2,"replicas":1,"readyReplicas":1,"availableReplicas":1}});
    let projected: DeploymentProjection = serde_json::from_value(value.clone()).unwrap();
    let full: Deployment = serde_json::from_value(value).unwrap();
    for deployment in [full, projected.into_resource()] {
        assert_eq!(
            builtin_summary("dev", &deployment, Utc::now())
                .status
                .as_deref(),
            Some("Rolling out")
        );
    }
}

#[test]
fn replica_set_startup_is_distinct_from_a_scaled_down_revision() {
    for (replicas, ready, expected) in [
        (1, 0, "Starting"),
        (1, 1, "Available: 1"),
        (0, 0, "Available: 0"),
    ] {
        let value = json!({"metadata":{"name":"api"},
            "status":{"replicas":replicas,"readyReplicas":ready,"availableReplicas":ready}});
        let projected: ReplicaSetProjection = serde_json::from_value(value.clone()).unwrap();
        let full: ReplicaSet = serde_json::from_value(value).unwrap();
        for replica_set in [full, projected.into_resource()] {
            assert_eq!(
                builtin_summary("dev", &replica_set, Utc::now())
                    .status
                    .as_deref(),
                Some(expected)
            );
        }
    }
}

#[test]
fn replica_set_startup_uses_desired_replicas_before_the_controller_catches_up() {
    for status in [
        serde_json::Value::Null,
        json!({"replicas":0}),
        json!({"replicas":1,"readyReplicas":1,"availableReplicas":1}),
    ] {
        let ready = status["readyReplicas"].as_i64().unwrap_or(0);
        let value = json!({"metadata":{"name":"api"},
            "spec":{"replicas":3,"selector":{},"template":{}},"status":status});
        let projected: ReplicaSetProjection = serde_json::from_value(value.clone()).unwrap();
        let full: ReplicaSet = serde_json::from_value(value).unwrap();
        for replica_set in [full, projected.into_resource()] {
            let summary = builtin_summary("dev", &replica_set, Utc::now());
            assert_eq!(summary.status.as_deref(), Some("Starting"));
            assert_eq!(summary.ready, Some(format!("{ready}/3")));
        }
    }
}

#[test]
fn deployment_creation_and_replica_set_scale_down_remain_in_progress() {
    let value = json!({"metadata":{"name":"api","generation":1},
        "spec":{"replicas":1,"selector":{},"template":{}}});
    let projected: DeploymentProjection = serde_json::from_value(value.clone()).unwrap();
    let full: Deployment = serde_json::from_value(value).unwrap();
    for deployment in [full, projected.into_resource()] {
        let summary = builtin_summary("dev", &deployment, Utc::now());
        assert_eq!(summary.status.as_deref(), Some("Rolling out"));
        assert_eq!(summary.ready.as_deref(), Some("0/1"));
    }
    let value = json!({"metadata":{"name":"api"},
        "spec":{"replicas":1,"selector":{},"template":{}},
        "status":{"replicas":2,"readyReplicas":1,"availableReplicas":1}});
    let projected: ReplicaSetProjection = serde_json::from_value(value.clone()).unwrap();
    let full: ReplicaSet = serde_json::from_value(value).unwrap();
    for replica_set in [full, projected.into_resource()] {
        assert_eq!(
            builtin_summary("dev", &replica_set, Utc::now())
                .status
                .as_deref(),
            Some("Scaling down")
        );
    }
}
