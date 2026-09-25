use super::super::manifest::manifest_summary;
use super::super::values::values_summary;
use super::*;
use std::collections::BTreeMap;

#[test]
fn missing_helm_owner_label_rejects_storage() {
    let labels = BTreeMap::from([("app".to_string(), "not-helm".to_string())]);

    assert!(!is_helm_owned(Some(&labels)));
    assert!(!is_helm_owned(None));
}

#[test]
fn values_summary_exposes_only_value_keys() {
    let config = serde_json::json!({
        "image": { "tag": "2026.5.22" },
        "replicaCount": 2,
    });

    let summary = values_summary(Some(&config));

    assert!(summary.has_values);
    assert_eq!(summary.value_count, 2);
    assert_eq!(summary.top_level_keys, vec!["image", "replicaCount"]);
}

#[test]
fn values_summary_treats_explicit_null_as_empty() {
    let summary = values_summary(Some(&serde_json::Value::Null));

    assert!(!summary.has_values);
    assert_eq!(summary.value_count, 0);
    assert!(summary.top_level_keys.is_empty());
}

#[test]
fn manifest_summary_preserves_missing_namespace_without_kind_guessing() {
    let manifest = r"
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: payments-reader
---
apiVersion: networking.k8s.io/v1
kind: IngressClass
metadata:
  name: internal
";

    let summary = manifest_summary(Some(manifest), Some("payments"));

    assert_eq!(summary.resource_count, 2);
    assert_eq!(summary.resources[0].kind.as_deref(), Some("ClusterRole"));
    assert_eq!(summary.resources[0].namespace, None);
    assert_eq!(summary.resources[1].kind.as_deref(), Some("IngressClass"));
    assert_eq!(summary.resources[1].namespace, None);
}

#[test]
fn rejects_decoded_release_over_approved_budget() {
    use std::io::Write;
    const LIMIT: usize = 32 * 1024 * 1024;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(b"{\"manifest\":\"").unwrap();
    let block = [b'x'; 1024];
    for _ in 0..LIMIT / block.len() {
        encoder.write_all(&block).unwrap();
    }
    encoder.write_all(b"\"}").unwrap();
    let payload = STANDARD.encode(encoder.finish().unwrap());
    let error = decode_helm_release(payload.as_bytes()).unwrap_err();
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(error.message.contains("32 MiB"));
}

type MockHandle =
    tower_test::mock::Handle<http::Request<kube::client::Body>, http::Response<kube::client::Body>>;

/// Serves list requests by path until the client under test is dropped.
async fn serve(mut handle: MockHandle, route: fn(&str) -> (u16, serde_json::Value)) {
    while let Some((request, send)) = handle.next_request().await {
        let (status, body) = route(request.uri().path());
        send.send_response(
            http::Response::builder()
                .status(status)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(kube::client::Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        );
    }
}

async fn list_with_routes(
    route: fn(&str) -> (u16, serde_json::Value),
) -> Result<HelmReleaseList, AppError> {
    let (service, handle) = tower_test::mock::pair();
    let client = Client::new(service, "default");
    let listing = list_helm_releases_with_client(client, "kind-dev", "default");
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(listing, serve(handle, route))
    })
    .await
    .expect("mock API responses timed out");
    result
}

fn list(items: serde_json::Value) -> (u16, serde_json::Value) {
    (
        200,
        serde_json::json!({ "apiVersion": "v1", "kind": "List", "metadata": {}, "items": items }),
    )
}

fn forbidden(resource: &str) -> (u16, serde_json::Value) {
    (
        403,
        serde_json::json!({
            "apiVersion": "v1", "kind": "Status", "metadata": {}, "status": "Failure",
            "message": format!("{resource} is forbidden"), "reason": "Forbidden", "code": 403
        }),
    )
}

fn namespaces() -> (u16, serde_json::Value) {
    list(
        serde_json::json!([{ "metadata": { "name": "apps" } }, { "metadata": { "name": "locked" } }]),
    )
}

fn configmap_release() -> (u16, serde_json::Value) {
    let release = STANDARD.encode(br#"{"name":"web","namespace":"apps","version":1}"#);
    list(serde_json::json!([{
        "metadata": {
            "name": "sh.helm.release.v1.web.v1",
            "namespace": "apps",
            "labels": { "owner": "helm", "name": "web", "status": "deployed", "version": "1" }
        },
        "data": { "release": release }
    }]))
}

#[tokio::test]
async fn forbidden_secret_storage_is_reported_beside_configmap_releases() {
    let listed = list_with_routes(|path| match path {
        "/api/v1/namespaces" => namespaces(),
        "/api/v1/configmaps" => configmap_release(),
        _ => forbidden("secrets"),
    })
    .await
    .expect("ConfigMap releases remain visible");

    assert_eq!(listed.releases.len(), 1);
    assert_eq!(listed.releases[0].name, "web");
    assert_eq!(
        listed.warnings,
        vec!["Helm Secret storage unavailable: forbidden by RBAC.".to_string()]
    );
}

#[tokio::test]
async fn namespaces_skipped_by_the_fallback_are_reported() {
    let listed = list_with_routes(|path| match path {
        "/api/v1/namespaces" => namespaces(),
        "/api/v1/namespaces/apps/secrets" | "/api/v1/configmaps" => configmap_release(),
        _ => forbidden("secrets"),
    })
    .await
    .expect("partial Secret fallback succeeds");

    assert_eq!(
        listed.warnings,
        vec!["Helm Secret storage unavailable in namespaces: locked.".to_string()]
    );
}

#[tokio::test]
async fn unreadable_storage_without_releases_stays_an_error() {
    let error = list_with_routes(|path| match path {
        "/api/v1/namespaces" => namespaces(),
        "/api/v1/configmaps" => list(serde_json::json!([])),
        _ => forbidden("secrets"),
    })
    .await
    .expect_err("an empty list must not hide unreadable storage");

    assert!(error.message.contains("forbidden"));
}

#[test]
fn supports_uncompressed_legacy_helm_release() {
    let encoded = STANDARD.encode(br#"{"name":"legacy","namespace":"default"}"#);
    let decoded = decode_helm_release(encoded.as_bytes()).expect("legacy release");
    assert_eq!(decoded.name.as_deref(), Some("legacy"));
}
