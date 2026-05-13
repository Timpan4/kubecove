use k8s_manager_lib::models::{AppError, ClusterContext, NamespaceSummary, ResourceSummary, ResourceDetails, ResourceDetailsFull, ResourceEventSummary};
use serde_json::json;

#[test]
fn test_app_error_new() {
    let err = AppError::new("test message", "test_kind");
    assert_eq!(err.message, "test message");
    assert_eq!(err.kind, "test_kind");
}

#[test]
fn test_app_error_kube() {
    let err = AppError::kube("kube error");
    assert_eq!(err.message, "kube error");
    assert_eq!(err.kind, "cluster");
}

#[test]
fn test_cluster_context_serde() {
    let ctx = ClusterContext { name: "minikube".to_string(), is_current: true };
    let json_str = serde_json::to_string(&ctx).unwrap();
    let parsed: ClusterContext = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.name, "minikube");
    assert!(parsed.is_current);
}

#[test]
fn test_namespace_summary_serde() {
    let ns = NamespaceSummary {
        name: "default".to_string(),
        age: "2024-01-01T00:00:00Z".to_string(),
        created_at: None,
    };
    let json_str = serde_json::to_string(&ns).unwrap();
    let parsed: NamespaceSummary = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.name, "default");
    assert_eq!(parsed.age, "2024-01-01T00:00:00Z");
}

#[test]
fn test_resource_summary_serde() {
    let rs = ResourceSummary {
        kind: "Pod".to_string(),
        cluster: "minikube".to_string(),
        name: "nginx".to_string(),
        namespace: Some("default".to_string()),
        age: "2024-01-01T00:00:00Z".to_string(),
        created_at: None,
        status: None,
        ready: None,
        restarts: None,
        owner_ref: None,
        argo_app: None,
        helm_release: None,
    };
    let json_str = serde_json::to_string(&rs).unwrap();
    let parsed: ResourceSummary = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.kind, "Pod");
    assert_eq!(parsed.namespace, Some("default".to_string()));
    assert_eq!(parsed.cluster, "minikube");
}

#[test]
fn test_resource_details_serde() {
    let rd = ResourceDetails {
        kind: "Pod".to_string(),
        cluster: "minikube".to_string(),
        name: "nginx".to_string(),
        namespace: Some("default".to_string()),
        yaml: "apiVersion: v1\nkind: Pod".to_string(),
    };
    let json_str = serde_json::to_string(&rd).unwrap();
    let parsed: ResourceDetails = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.yaml, "apiVersion: v1\nkind: Pod");
    assert_eq!(parsed.kind, "Pod");
}

#[test]
fn test_app_error_serialize() {
    let err = AppError::kube("connection refused");
    let json_val = serde_json::to_value(&err).unwrap();
    assert_eq!(json_val["message"], "connection refused");
    assert_eq!(json_val["kind"], "cluster");
}

#[test]
fn test_cluster_context_from_json() {
    let json_val = json!({ "name": "docker-desktop", "isCurrent": false });
    let ctx: ClusterContext = serde_json::from_value(json_val).unwrap();
    assert_eq!(ctx.name, "docker-desktop");
    assert!(!ctx.is_current);
}

#[test]
fn test_resource_details_full_serde() {
    let summary = ResourceSummary {
        kind: "Pod".to_string(),
        cluster: "minikube".to_string(),
        name: "nginx".to_string(),
        namespace: Some("default".to_string()),
        age: "2024-01-01T00:00:00Z".to_string(),
        created_at: None,
        status: None,
        ready: None,
        restarts: None,
        owner_ref: None,
        argo_app: None,
        helm_release: None,
    };
    let rdf = ResourceDetailsFull {
        summary,
        yaml: "apiVersion: v1\nkind: Pod".to_string(),
        metadata: serde_json::json!({ "name": "nginx", "namespace": "default" }),
        status: Some(serde_json::json!({ "phase": "Running" })),
    };
    let json_str = serde_json::to_string(&rdf).unwrap();
    let parsed: ResourceDetailsFull = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.yaml, "apiVersion: v1\nkind: Pod");
    assert_eq!(parsed.summary.kind, "Pod");
    assert!(parsed.status.is_some());
}

#[test]
fn test_resource_event_summary_serde() {
    let event = ResourceEventSummary {
        event_type: "Warning".to_string(),
        reason: "BackOff".to_string(),
        message: "Back-off restarting failed container".to_string(),
        count: 3,
        last_seen: "2m".to_string(),
        last_seen_at: None,
        source: "kubelet".to_string(),
        namespace: Some("default".to_string()),
    };
    let json_val = serde_json::to_value(&event).unwrap();
    assert_eq!(json_val["eventType"], "Warning");
    assert_eq!(json_val["reason"], "BackOff");
    let parsed: ResourceEventSummary = serde_json::from_value(json_val).unwrap();
    assert_eq!(parsed.count, 3);
    assert_eq!(parsed.namespace, Some("default".to_string()));
}
