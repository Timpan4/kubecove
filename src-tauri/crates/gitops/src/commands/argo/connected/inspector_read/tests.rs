use super::*;
use crate::commands::argo::transport::url;
use crate::models::ArgoManagedResource;

#[test]
fn connected_paths_encode_application_and_query_identity() {
    let path = connected_application_path(
        "https://argo.example/argo-cd",
        "a/b?#",
        Some("ns &/#?"),
        Some("project&?#/"),
        false,
    )
    .unwrap();
    assert_eq!(
        path,
        "/api/v1/applications/a%2Fb%3F%23?appNamespace=ns+%26%2F%23%3F&project=project%26%3F%23%2F"
    );
}

#[test]
fn connected_paths_preserve_base_path_for_managed_resources() {
    assert_eq!(
        connected_application_path(
            "https://argo.example/argo-cd",
            "demo",
            Some("argocd"),
            None,
            true,
        )
        .unwrap(),
        "/api/v1/applications/demo/managed-resources?appNamespace=argocd"
    );
}

#[test]
fn connected_paths_apply_argo_root_path_once() {
    let path = connected_application_path(
        "https://argo.example/argo",
        "demo",
        Some("argocd"),
        None,
        false,
    )
    .unwrap();

    assert_eq!(
        url("https://argo.example/argo", &path).unwrap(),
        "https://argo.example/argo/api/v1/applications/demo?appNamespace=argocd"
    );
}

#[test]
fn managed_resource_paths_filter_by_full_identity() {
    let application = ArgoApplicationRef {
        name: "platform".into(),
        namespace: Some("argocd".into()),
        ..Default::default()
    };
    let core = ArgoManagedResource {
        version: Some("v1".into()),
        kind: Some("Service".into()),
        namespace: Some("argo cd".into()),
        name: Some("argocd-server&x".into()),
        ..Default::default()
    };
    assert_eq!(
        managed_resource_path("https://argo.example", &application, &core).unwrap(),
        "/api/v1/applications/platform/managed-resources?appNamespace=argocd&namespace=argo+cd&name=argocd-server%26x&version=v1&kind=Service"
    );
    let cluster_scoped = ArgoManagedResource {
        group: Some("apiextensions.k8s.io".into()),
        version: Some("v1".into()),
        kind: Some("CustomResourceDefinition".into()),
        name: Some("applicationsets.argoproj.io".into()),
        ..Default::default()
    };
    assert_eq!(
        managed_resource_path("https://argo.example", &application, &cluster_scoped).unwrap(),
        "/api/v1/applications/platform/managed-resources?appNamespace=argocd&name=applicationsets.argoproj.io&version=v1&group=apiextensions.k8s.io&kind=CustomResourceDefinition"
    );
}

#[test]
fn fallback_keeps_complete_kubernetes_provenance_visible() {
    let inspector = ArgoApplicationInspector {
        transport: "kubernetes".into(),
        provenance: "kubernetes-status-no-diff".into(),
        resources: vec![ArgoManagedResource {
            kind: Some("Deployment".into()),
            name: Some("api".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fallback = with_connected_fallback(
        inspector,
        &AppError::new("token=plaintext", AppErrorKind::ArgoConnection),
    );

    assert_eq!(fallback.transport, "kubernetes");
    assert_eq!(fallback.provenance, "kubernetes-status-no-diff");
    assert_eq!(fallback.resources.len(), 1);
    let connected = fallback.connected_fallback.expect("fallback metadata");
    assert_eq!(connected.transport, "connected");
    assert_eq!(connected.failure.kind, "argoConnection");
    assert_eq!(connected.failure.message, "connection unavailable");
}

#[test]
fn fallback_and_combined_errors_are_sanitized() {
    let connected = AppError::new(
        "token=plaintext https://user:password@argo.example/api kubeconfig /tmp/config",
        AppErrorKind::ArgoConnection,
    );
    let kubernetes = AppError::new("Secret data: plaintext", AppErrorKind::Cluster);
    let fallback = inspection_failure(&connected);
    let error = inspection_error(&connected, &kubernetes);

    assert_eq!(fallback.kind, "argoConnection");
    assert_eq!(fallback.message, "connection unavailable");
    assert_eq!(error.kind, AppErrorKind::ArgoInspection);
    assert!(error.message.contains("argoConnection"));
    assert!(error.message.contains("cluster"));
    assert!(!error.message.contains("plaintext"));
    assert!(!error.message.contains("https://"));
}
