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

#[tokio::test]
async fn manifest_and_label_reads_overlap_without_changing_duplicate_precedence() {
    let (service, mut handle) = tower_test::mock::pair();
    let client = Client::new(service, "payments");
    let release = HelmReleaseSummary {
        cluster: "test".into(),
        name: "web".into(),
        namespace: "payments".into(),
        age: String::new(),
        updated_at: None,
        created_at: None,
        chart: None,
        app_version: None,
        revision: None,
        status: None,
        storage_kind: "Secret".into(),
        storage_name: "web.v1".into(),
    };
    let manifest = [None, Some("payments")]
        .map(|namespace| manifest_ref(Some("apps/v1"), Some("Deployment"), namespace, Some("web")));
    let discovered = [
        deployment_kind(),
        DiscoveredResourceKind {
            group: String::new(),
            api_version: "v1".into(),
            kind: "ConfigMap".into(),
            plural: "configmaps".into(),
            version: "v1".into(),
            ..deployment_kind()
        },
    ];
    let reconcile = reconcile_release(client, "test", &release, &manifest, &discovered);
    let serve = async {
        let object = |name, uid| {
            serde_json::json!({"apiVersion": "apps/v1", "kind": "Deployment", "metadata": {
                "name": name, "uid": uid, "namespace": "payments", "labels": {"helm.sh/release": "web"}
            }})
        };
        let forbidden = || {
            serde_json::json!({"apiVersion": "v1", "kind": "Status", "status": "Failure",
                "message": "forbidden", "reason": "Forbidden", "code": 403})
        };
        let mut first_get = true;
        let mut responses = Vec::new();
        // Hold all four responses until both GETs and both LISTs have reached the API.
        for _ in 0..4 {
            let (request, response) = handle.next_request().await.expect("reconciliation read");
            let (status, body): (u16, _) = match request.uri().path() {
                "/apis/apps/v1/namespaces/payments/deployments/web" => {
                    if std::mem::take(&mut first_get) {
                        (403, forbidden())
                    } else {
                        (200, object("web", "manifest"))
                    }
                }
                "/apis/apps/v1/namespaces/payments/deployments" => (
                    200,
                    serde_json::json!({
                        "apiVersion": "apps/v1", "kind": "DeploymentList", "metadata": {},
                            "items": [object("web", "label-only"), object("extra", "first"), object("extra", "second")]
                    }),
                ),
                "/api/v1/namespaces/payments/configmaps" => (403, forbidden()),
                path => panic!("unexpected mock request to {path}"),
            };
            responses.push((response, status, body));
        }
        for (response, status, body) in responses.into_iter().rev() {
            response.send_response(
                http::Response::builder()
                    .status(status)
                    .body(kube::client::Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            );
            tokio::task::yield_now().await;
        }
    };
    let ((rows, warnings, totals), ()) =
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(reconcile, serve)
        })
        .await
        .expect("mock API requests did not overlap");
    assert_eq!(
        rows.iter()
            .map(|row| (row.name.as_deref(), row.status))
            .collect::<Vec<_>>(),
        [
            (Some("extra"), HelmReconciliationStatus::LabelOnly),
            (Some("web"), HelmReconciliationStatus::Tracked)
        ]
    );
    assert_eq!(totals.label_only, 1);
    assert_eq!(totals.tracked, 1);
    assert_eq!(
        rows[0].live_resource.as_ref().unwrap().uid.as_deref(),
        Some("first")
    );
    assert_eq!(
        rows[1].live_resource.as_ref().unwrap().uid.as_deref(),
        Some("manifest")
    );
    assert_eq!(warnings.len(), 2);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("v1 ConfigMap for explicit Helm labels"))
    );
}

// Failure modes: an unrelated API publishing `"verbs": null` fails discovery,
// a manifest group whose aggregated API server is down fails the whole
// reconciliation instead of leaving only its own rows unavailable, an
// unavailable preferred version hides the group's available versions, a kind
// served only at a non-preferred version is missed, and a skipped version
// leaves the scan looking complete.
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
            kind("example.com", "v1alpha1", "Gizmo", "gizmos"),
            kind("example.com", "v1alpha1", "Widget", "widgets"),
        ],
        &[("down.example.com", "v1"), ("example.com", "v1beta1")],
    );
    let manifest = [
        manifest_ref(Some("example.com/v1"), Some("Widget"), None, Some("blue")),
        manifest_ref(
            Some("example.com/v1alpha1"),
            Some("Gizmo"),
            None,
            Some("green"),
        ),
        manifest_ref(
            Some("down.example.com/v1"),
            Some("Gadget"),
            None,
            Some("red"),
        ),
    ];

    let (discovered, warnings) = discovered_resources(client, &manifest)
        .await
        .expect("discovery");

    // `Widget` is served at v1 and v1alpha1 and is kept once, at v1.
    let kinds: Vec<(&str, &str)> = discovered
        .iter()
        .map(|kind| (kind.api_version.as_str(), kind.kind.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("apps/v1", "Deployment"),
            ("example.com/v1", "Widget"),
            ("example.com/v1alpha1", "Gizmo"),
            ("v1", "Service"),
        ]
    );
    // Skipped versions must read as an incomplete scan, not an empty one.
    assert_eq!(
        warnings,
        [
            "API down.example.com/v1 is unavailable, so resources at that version were not checked.",
            "API example.com/v1beta1 is unavailable, so resources at that version were not checked.",
        ]
    );
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
