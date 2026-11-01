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
        short_names: Vec::new(),
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
        short_names: Vec::new(),
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

// Failure modes: an unrelated API publishing `"verbs": null` fails discovery,
// a manifest group whose aggregated API server is down fails the whole
// reconciliation instead of leaving only its own rows unavailable, and one
// unavailable version of a group hides the group's available version.
#[tokio::test]
async fn discovery_covers_manifest_and_common_kinds_beside_null_verbs_and_unavailable_apis() {
    use kubecove_kube::commands::discovery_fixture::{self, FixtureKind};
    let kind = |group, version, kind, plural| FixtureKind {
        group,
        version,
        kind,
        plural,
        namespaced: true,
    };
    let client = discovery_fixture::client_with_unavailable_groups(
        &[
            kind("", "v1", "Service", "services"),
            kind("apps", "v1", "Deployment", "deployments"),
            kind("example.com", "v1", "Widget", "widgets"),
        ],
        &[("down.example.com", "v1"), ("example.com", "v1beta1")],
    );
    let manifest = [
        manifest_ref(Some("example.com/v1"), Some("Widget"), None, Some("blue")),
        manifest_ref(
            Some("down.example.com/v1"),
            Some("Gadget"),
            None,
            Some("red"),
        ),
    ];

    let discovered = discovered_resources(client, &manifest)
        .await
        .expect("discovery");

    let mut kinds: Vec<&str> = discovered.iter().map(|kind| kind.kind.as_str()).collect();
    kinds.sort_unstable();
    assert_eq!(kinds, ["Deployment", "Service", "Widget"]);
}

#[test]
fn conservative_label_scan_includes_common_namespaced_and_manifest_kinds() {
    let widget = DiscoveredResourceKind {
        group: "example.com".to_string(),
        version: "v1".to_string(),
        api_version: "example.com/v1".to_string(),
        kind: "Widget".to_string(),
        plural: "widgets".to_string(),
        short_names: Vec::new(),
        namespaced: true,
    };
    // Shares a common kind's name but not its group, so only a manifest
    // reference scans it.
    let knative_service = DiscoveredResourceKind {
        group: "serving.knative.dev".to_string(),
        version: "v1".to_string(),
        api_version: "serving.knative.dev/v1".to_string(),
        kind: "Service".to_string(),
        plural: "services".to_string(),
        short_names: Vec::new(),
        namespaced: true,
    };
    let cluster_role = cluster_role_kind();
    let discovered = vec![
        deployment_kind(),
        widget.clone(),
        cluster_role,
        knative_service,
    ];
    let manifest = BTreeSet::from([("example.com/v1".to_string(), "Widget".to_string())]);

    let kinds = conservative_label_scan_kinds(&discovered, &manifest);

    assert!(kinds.iter().any(|kind| kind.kind == "Deployment"));
    assert!(kinds.iter().any(|kind| kind.kind == "Widget"));
    assert!(!kinds.iter().any(|kind| kind.kind == "ClusterRole"));
    assert!(!kinds.iter().any(|kind| kind.group == "serving.knative.dev"));
}
