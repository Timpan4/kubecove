use super::super::builtin_summary::builtin_summary;
use super::*;
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

fn fixture(kind: &str, api_version: &str) -> Value {
    json!({
        "apiVersion": api_version, "kind": kind,
        "metadata": {
            "name": "workload", "namespace": "tenant", "uid": "uid",
            "creationTimestamp": "2026-10-01T00:00:00Z",
            "labels": {"helm.sh/release": "release", "app": "app"},
            "annotations": {"argocd.argoproj.io/tracking-id": "app:apps/Deployment:tenant/workload",
                "kubectl.kubernetes.io/last-applied-configuration": "large unused document"},
            "ownerReferences": [{"apiVersion":"apps/v1", "kind":"Deployment", "name":"parent", "uid":"parent", "controller":true}],
            "managedFields": [{"manager":"controller", "fieldsV1":{"f:spec":{}}}]
        },
        "spec": {"completions": 3, "selector": {}, "template": {"spec": {"containers": [{"name":"app", "image":"app", "env":[{"name":"UNUSED", "value":"large value"}]}]}}},
        "status": {"phase":"Running", "conditions":[{"type":"Ready", "status":"True"}],
            "containerStatuses":[{"name":"app", "image":"app", "imageID":"id", "ready":true, "restartCount":2}],
            "availableReplicas":2, "readyReplicas":1, "replicas":3,
            "active":1, "failed":0, "succeeded":2}
    })
}

#[test]
fn projected_summaries_equal_full_decode_with_missing_null_and_populated_fields() {
    let now = Utc.with_ymd_and_hms(2026, 10, 21, 0, 0, 0).unwrap();
    macro_rules! compare {
        ($kind:literal, $version:literal, $full:ty, $projection:ty) => {
            for variant in 0..4 {
                let mut value = fixture($kind, $version);
                if variant == 1 {
                    value.as_object_mut().unwrap().remove("status");
                }
                if variant == 2 {
                    value["status"] = Value::Null;
                    value["spec"] = Value::Null;
                    value["metadata"] = Value::Null;
                }
                if variant == 3 {
                    value["status"] = json!({});
                    value.as_object_mut().unwrap().remove("spec");
                }
                let full: $full = serde_json::from_value(value.clone()).unwrap();
                let projected: $projection = serde_json::from_value(value).unwrap();
                let projected = projected.into_resource();
                assert_eq!(
                    serde_json::to_vec(&builtin_summary("context", &full, now)).unwrap(),
                    serde_json::to_vec(&builtin_summary("context", &projected, now)).unwrap(),
                    "{} variant {}",
                    $kind,
                    variant
                );
                assert!(projected.metadata.managed_fields.is_none());
                assert!(!projected.metadata.annotations.as_ref().is_some_and(
                    |a| a.contains_key("kubectl.kubernetes.io/last-applied-configuration")
                ));
            }
        };
    }
    compare!("Pod", "v1", Pod, PodProjection);
    compare!("Deployment", "apps/v1", Deployment, DeploymentProjection);
    compare!("ReplicaSet", "apps/v1", ReplicaSet, ReplicaSetProjection);
    compare!("Job", "batch/v1", Job, JobProjection);
}

#[test]
fn projections_reject_wrong_typed_identity_and_invalid_retained_counts() {
    for field in ["apiVersion", "kind"] {
        for invalid in [json!("wrong"), Value::Null, json!(12)] {
            let mut value = fixture("Pod", "v1");
            value[field] = invalid;
            assert!(serde_json::from_value::<Pod>(value.clone()).is_err());
            assert!(serde_json::from_value::<PodProjection>(value).is_err());
        }
    }
    let mut value = fixture("Pod", "v1");
    value["status"]["containerStatuses"][0]["restartCount"] = json!("invalid");
    assert!(serde_json::from_value::<PodProjection>(value).is_err());
}

#[test]
fn projected_dynamic_objects_preserve_summary_and_topology() {
    use kubecove_kube::commands::list_projection::Dynamic;
    let kind = crate::models::DiscoveredResourceKind {
        group: "example.com".into(),
        version: "v1".into(),
        api_version: "example.com/v1".into(),
        kind: "Widget".into(),
        plural: "widgets".into(),
        short_names: vec![],
        namespaced: true,
    };
    let now = Utc.with_ymd_and_hms(2026, 10, 21, 0, 0, 0).unwrap();
    for status in [
        Value::Null,
        json!({}),
        json!({"phase":""}),
        json!({"conditions":[{"type":"Ready", "status":"False", "message":"failure", "reason":"Failed", "observedGeneration":2}]}),
    ] {
        let mut value = fixture("Widget", "example.com/v1");
        value["status"] = status;
        let full = serde_json::from_value(value.clone()).unwrap();
        let projected: kube::api::DynamicObject =
            serde_json::from_value::<Dynamic>(value).unwrap().into();
        let full =
            super::super::topology_dynamic::dynamic_topology_input_at("context", &kind, &full, now);
        let projected = super::super::topology_dynamic::dynamic_topology_input_at(
            "context", &kind, &projected, now,
        );
        assert_eq!(
            serde_json::to_vec(&full.summary).unwrap(),
            serde_json::to_vec(&projected.summary).unwrap()
        );
        assert_eq!(full.uid, projected.uid);
        assert_eq!(full.labels, projected.labels);
        assert_eq!(full.owner, projected.owner);
    }
}
