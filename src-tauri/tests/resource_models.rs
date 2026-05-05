use k8s_manager_lib::models::{ResourceDetails, ResourceSummary};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn resource_summary_serializes_frontend_contract_fields() {
    let labels = BTreeMap::from([(
        "app.kubernetes.io/instance".to_string(),
        "demo".to_string(),
    )]);
    let summary = ResourceSummary {
        cluster_context: "kind-dev".to_string(),
        api_version: "v1".to_string(),
        kind: "Pod".to_string(),
        namespace: Some("default".to_string()),
        name: "demo-pod".to_string(),
        uid: Some("pod-uid".to_string()),
        status: Some("Running".to_string()),
        age: Some("5m".to_string()),
        ready: Some("1/1".to_string()),
        restarts: Some(0),
        owner: Some("Deployment/demo".to_string()),
        argo_app: Some("demo-app".to_string()),
        helm_release: Some("demo".to_string()),
        labels: Some(labels),
    };

    let value = serde_json::to_value(summary).expect("resource summary should serialize");

    assert_eq!(
        value,
        json!({
            "clusterContext": "kind-dev",
            "apiVersion": "v1",
            "kind": "Pod",
            "namespace": "default",
            "name": "demo-pod",
            "uid": "pod-uid",
            "status": "Running",
            "age": "5m",
            "ready": "1/1",
            "restarts": 0,
            "owner": "Deployment/demo",
            "argoApp": "demo-app",
            "helmRelease": "demo",
            "labels": {
                "app.kubernetes.io/instance": "demo"
            }
        })
    );
}

#[test]
fn resource_details_keeps_yaml_metadata_and_status_read_only() {
    let summary = ResourceSummary {
        cluster_context: "kind-dev".to_string(),
        api_version: "v1".to_string(),
        kind: "ConfigMap".to_string(),
        namespace: Some("default".to_string()),
        name: "demo-config".to_string(),
        uid: None,
        status: None,
        age: None,
        ready: None,
        restarts: None,
        owner: None,
        argo_app: None,
        helm_release: None,
        labels: None,
    };
    let details = ResourceDetails {
        summary,
        yaml: "apiVersion: v1\nkind: ConfigMap\n".to_string(),
        metadata: json!({ "name": "demo-config" }),
        status: None,
    };

    let value = serde_json::to_value(details).expect("resource details should serialize");

    assert_eq!(value["yaml"], "apiVersion: v1\nkind: ConfigMap\n");
    assert_eq!(value["metadata"], json!({ "name": "demo-config" }));
    assert!(value.get("status").is_none());
}
