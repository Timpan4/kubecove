use kubecove_lib::models::{
    AppError, ArgoAppProjectDetails, ArgoAppProjectSummary, ArgoApplicationDetails,
    ArgoApplicationSetDetails, ArgoApplicationSetSummary, ArgoApplicationSummary, ClusterContext,
    DiscoveredResourceKind, NamespaceSummary, PodLogStreamRequest, ResourceDetails,
    ResourceDetailsFull, ResourceEventSummary, ResourceSummary, StreamMessage, WatchResourceKey,
    WatchResourceKind,
};
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
    let ctx = ClusterContext {
        name: "minikube".to_string(),
        is_current: true,
    };
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
        api_version: None,
        group: None,
        version: None,
        plural: None,
        namespaced: None,
        dynamic: None,
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
        api_version: None,
        group: None,
        version: None,
        plural: None,
        namespaced: None,
        dynamic: None,
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

#[test]
fn test_discovered_resource_kind_serde() {
    let kind = DiscoveredResourceKind {
        group: "apps".to_string(),
        version: "v1".to_string(),
        api_version: "apps/v1".to_string(),
        kind: "Deployment".to_string(),
        plural: "deployments".to_string(),
        namespaced: true,
    };
    let json_val = serde_json::to_value(&kind).unwrap();
    assert_eq!(json_val["apiVersion"], "apps/v1");
    assert_eq!(json_val["namespaced"], true);
    let parsed: DiscoveredResourceKind = serde_json::from_value(json_val).unwrap();
    assert_eq!(parsed.kind, "Deployment");
}

#[test]
fn test_crd_discovered_resource_kind_serde() {
    let kind = DiscoveredResourceKind {
        group: "apiextensions.k8s.io".to_string(),
        version: "v1".to_string(),
        api_version: "apiextensions.k8s.io/v1".to_string(),
        kind: "CustomResourceDefinition".to_string(),
        plural: "customresourcedefinitions".to_string(),
        namespaced: false,
    };
    let json_val = serde_json::to_value(&kind).unwrap();
    assert_eq!(json_val["apiVersion"], "apiextensions.k8s.io/v1");
    assert_eq!(json_val["plural"], "customresourcedefinitions");
    assert_eq!(json_val["namespaced"], false);
    let parsed: DiscoveredResourceKind = serde_json::from_value(json_val).unwrap();
    assert_eq!(parsed.kind, "CustomResourceDefinition");
    assert!(!parsed.namespaced);
}

#[test]
fn test_dynamic_resource_summary_fields_serde() {
    let summary = ResourceSummary {
        kind: "Widget".to_string(),
        cluster: "kind-prod".to_string(),
        name: "sample".to_string(),
        namespace: Some("default".to_string()),
        age: "1h".to_string(),
        api_version: Some("example.com/v1".to_string()),
        group: Some("example.com".to_string()),
        version: Some("v1".to_string()),
        plural: Some("widgets".to_string()),
        namespaced: Some(true),
        dynamic: Some(true),
        created_at: None,
        status: Some("Running".to_string()),
        ready: None,
        restarts: None,
        owner_ref: Some("sample-owner".to_string()),
        argo_app: None,
        helm_release: None,
    };

    let json_val = serde_json::to_value(&summary).unwrap();
    assert_eq!(json_val["apiVersion"], "example.com/v1");
    assert_eq!(json_val["dynamic"], true);
    assert_eq!(json_val["plural"], "widgets");
    let parsed: ResourceSummary = serde_json::from_value(json_val).unwrap();
    assert_eq!(parsed.group, Some("example.com".to_string()));
    assert_eq!(parsed.namespaced, Some(true));
    assert_eq!(parsed.owner_ref, Some("sample-owner".to_string()));
}

#[test]
fn test_argo_application_models_serde() {
    let summary = ArgoApplicationSummary {
        cluster: "kind-prod".to_string(),
        name: "payments".to_string(),
        age: "1h".to_string(),
        created_at: Some("2026-05-15T10:00:00Z".to_string()),
        namespace: Some("argocd".to_string()),
        project: Some("default".to_string()),
        sync_status: Some("Synced".to_string()),
        health_status: Some("Healthy".to_string()),
        destination_namespace: Some("payments".to_string()),
        destination_server: Some("https://kubernetes.default.svc".to_string()),
        source_repo: Some("https://example.invalid/repo.git".to_string()),
        source_revision: Some("main".to_string()),
    };
    let details = ArgoApplicationDetails {
        summary,
        yaml: "kind: Application".to_string(),
        metadata: json!({ "name": "payments" }),
        status: Some(json!({ "sync": { "status": "Synced" } })),
    };

    let json_val = serde_json::to_value(&details).unwrap();
    assert_eq!(json_val["summary"]["syncStatus"], "Synced");
    let parsed: ArgoApplicationDetails = serde_json::from_value(json_val).unwrap();
    assert_eq!(parsed.summary.health_status, Some("Healthy".to_string()));
}

#[test]
fn test_argo_appset_models_serde() {
    let summary = ArgoApplicationSetSummary {
        cluster: "kind-prod".to_string(),
        name: "payments-set".to_string(),
        age: "2h".to_string(),
        created_at: None,
        namespace: Some("argocd".to_string()),
        project: Some("default".to_string()),
        status: Some("ResourcesUpToDate".to_string()),
        sync_status: Some("Synced".to_string()),
        health_status: Some("Healthy".to_string()),
        destination_namespace: Some("payments".to_string()),
        destination_server: Some("https://kubernetes.default.svc".to_string()),
        source_repo: Some("https://example.invalid/repo.git".to_string()),
        source_revision: Some("main".to_string()),
    };
    let details = ArgoApplicationSetDetails {
        summary,
        yaml: "kind: ApplicationSet".to_string(),
        metadata: json!({ "name": "payments-set" }),
    };

    let json_val = serde_json::to_value(&details).unwrap();
    assert_eq!(json_val["summary"]["healthStatus"], "Healthy");
    let parsed: ArgoApplicationSetDetails = serde_json::from_value(json_val).unwrap();
    assert_eq!(parsed.summary.status, Some("ResourcesUpToDate".to_string()));
}

#[test]
fn test_argo_appproject_models_serde() {
    let summary = ArgoAppProjectSummary {
        cluster: "kind-prod".to_string(),
        name: "default".to_string(),
        age: "3h".to_string(),
        created_at: None,
        namespace: Some("argocd".to_string()),
        description: Some("default project".to_string()),
        status: Some("Healthy".to_string()),
    };
    let details = ArgoAppProjectDetails {
        summary,
        yaml: "kind: AppProject".to_string(),
        metadata: json!({ "name": "default" }),
    };

    let json_val = serde_json::to_value(&details).unwrap();
    assert_eq!(json_val["summary"]["description"], "default project");
    let parsed: ArgoAppProjectDetails = serde_json::from_value(json_val).unwrap();
    assert_eq!(parsed.summary.name, "default");
}

#[test]
fn test_stream_models_serde() {
    let key = WatchResourceKey {
        resource_kind: WatchResourceKind {
            kind: "Pod".to_string(),
            group: None,
            version: None,
            api_version: None,
            plural: None,
            namespaced: None,
        },
        namespace: Some("default".to_string()),
    };
    let json_val = serde_json::to_value(&key).unwrap();
    assert_eq!(json_val["resourceKind"]["kind"], "Pod");
    assert_eq!(json_val["namespace"], "default");

    let request = PodLogStreamRequest {
        cluster_context: "admin@solid-k8s".to_string(),
        namespace: "default".to_string(),
        pod_name: "api-0".to_string(),
        container: Some("api".to_string()),
        tail_lines: Some(200),
    };
    let json_val = serde_json::to_value(&request).unwrap();
    assert_eq!(json_val["clusterContext"], "admin@solid-k8s");
    assert_eq!(json_val["podName"], "api-0");
    assert_eq!(json_val["tailLines"], 200);

    let message = StreamMessage::LogLine {
        stream_id: "logs-1".to_string(),
        line: "2026-05-18T10:00:00Z ready".to_string(),
    };
    let json_val = serde_json::to_value(&message).unwrap();
    assert_eq!(json_val["type"], "logLine");
    assert_eq!(json_val["streamId"], "logs-1");
}
