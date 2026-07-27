use super::*;
use crate::models::DiscoveredResourceKind;
use std::collections::BTreeSet;

fn deployment_kind() -> DiscoveredResourceKind {
    DiscoveredResourceKind {
        group: "apps".to_string(),
        version: "v1".to_string(),
        api_version: "apps/v1".to_string(),
        kind: "Deployment".to_string(),
        plural: "deployments".to_string(),
        namespaced: true,
    }
}

fn cluster_role_kind() -> DiscoveredResourceKind {
    DiscoveredResourceKind {
        group: "rbac.authorization.k8s.io".to_string(),
        version: "v1".to_string(),
        api_version: "rbac.authorization.k8s.io/v1".to_string(),
        kind: "ClusterRole".to_string(),
        plural: "clusterroles".to_string(),
        namespaced: false,
    }
}

fn manifest_ref(
    api_version: Option<&str>,
    kind: Option<&str>,
    namespace: Option<&str>,
    name: Option<&str>,
) -> HelmManifestResourceSummary {
    HelmManifestResourceSummary {
        api_version: api_version.map(str::to_string),
        kind: kind.map(str::to_string),
        namespace: namespace.map(str::to_string),
        name: name.map(str::to_string),
    }
}

#[test]
fn namespaced_manifest_refs_default_to_release_namespace() {
    let discovered = discovered_ref_index(&[deployment_kind()]);
    let identity = resource_identity_from_manifest(
        &manifest_ref(Some("apps/v1"), Some("Deployment"), None, Some("api")),
        "payments",
        &discovered,
    )
    .expect("identity");

    assert_eq!(identity.key.namespace.as_deref(), Some("payments"));
}

#[test]
fn cluster_scoped_manifest_refs_keep_empty_namespace() {
    let discovered = discovered_ref_index(&[cluster_role_kind()]);
    let identity = resource_identity_from_manifest(
        &manifest_ref(
            Some("rbac.authorization.k8s.io/v1"),
            Some("ClusterRole"),
            Some("ignored"),
            Some("reader"),
        ),
        "payments",
        &discovered,
    )
    .expect("identity");

    assert_eq!(identity.key.namespace, None);
}

#[test]
fn unavailable_status_covers_missing_manifest_identity() {
    let discovered = discovered_ref_index(&[deployment_kind()]);
    let err = resource_identity_from_manifest(
        &manifest_ref(Some("apps/v1"), Some("Deployment"), None, None),
        "payments",
        &discovered,
    )
    .expect_err("missing name");

    assert_eq!(err, "manifest resource is missing name");
}

#[test]
fn reconciliation_totals_count_each_status() {
    let rows = vec![
        HelmReconciliationResource {
            api_version: Some("apps/v1".to_string()),
            kind: Some("Deployment".to_string()),
            namespace: Some("payments".to_string()),
            name: Some("api".to_string()),
            status: HelmReconciliationStatus::Tracked,
            status_message: String::new(),
            in_manifest: true,
            explicit_helm_label: true,
            live_resource: None,
        },
        HelmReconciliationResource {
            api_version: Some("v1".to_string()),
            kind: Some("Service".to_string()),
            namespace: Some("payments".to_string()),
            name: Some("api".to_string()),
            status: HelmReconciliationStatus::Missing,
            status_message: String::new(),
            in_manifest: true,
            explicit_helm_label: false,
            live_resource: None,
        },
        HelmReconciliationResource {
            api_version: Some("v1".to_string()),
            kind: Some("Secret".to_string()),
            namespace: Some("payments".to_string()),
            name: Some("extra".to_string()),
            status: HelmReconciliationStatus::LabelOnly,
            status_message: String::new(),
            in_manifest: false,
            explicit_helm_label: true,
            live_resource: None,
        },
    ];

    let totals = reconciliation_totals(&rows);

    assert_eq!(totals.tracked, 1);
    assert_eq!(totals.missing, 1);
    assert_eq!(totals.label_only, 1);
    assert_eq!(totals.unlabeled_live, 0);
    assert_eq!(totals.unavailable, 0);
}

#[test]
fn conservative_label_scan_includes_common_namespaced_and_manifest_kinds() {
    let widget = DiscoveredResourceKind {
        group: "example.com".to_string(),
        version: "v1".to_string(),
        api_version: "example.com/v1".to_string(),
        kind: "Widget".to_string(),
        plural: "widgets".to_string(),
        namespaced: true,
    };
    let cluster_role = cluster_role_kind();
    let discovered = vec![deployment_kind(), widget.clone(), cluster_role];
    let manifest = BTreeSet::from([("example.com/v1".to_string(), "Widget".to_string())]);

    let kinds = conservative_label_scan_kinds(&discovered, &manifest);

    assert!(kinds.iter().any(|kind| kind.kind == "Deployment"));
    assert!(kinds.iter().any(|kind| kind.kind == "Widget"));
    assert!(!kinds.iter().any(|kind| kind.kind == "ClusterRole"));
}
