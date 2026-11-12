use super::*;
use crate::commands::{ClientIntent, FixedClusterAccess};
use crate::models::AppErrorKind;
use http::{Method, Request, Response, header::CONTENT_TYPE};
use kube::{Client, client::Body};
use serde_json::json;
use std::{sync::Arc, time::Duration};

type MockHandle = tower_test::mock::Handle<Request<Body>, Response<Body>>;

/// A session whose requests resolve to a mock cluster, and that cluster's handle.
fn mock_cluster() -> (Arc<FixedClusterAccess>, MockHandle) {
    let (service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    (
        FixedClusterAccess::new(Client::new(service, "default")),
        handle,
    )
}

fn json_response(status: u16, body: serde_json::Value) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn service_manifest() -> serde_json::Value {
    json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": "api",
            "namespace": "default",
            "uid": "uid-1",
            "resourceVersion": "7"
        },
        "spec": {
            "ports": [{ "port": 80 }]
        }
    })
}

fn base_request_yaml(force_conflicts: bool) -> YamlApplyRequest {
    YamlApplyRequest {
        cluster_context: "kind-kind".to_string(),
        kubeconfig_env_var: None,
        kind: "Service".to_string(),
        api_version: Some("v1".to_string()),
        group: None,
        version: None,
        plural: None,
        namespaced: Some(true),
        name: "api".to_string(),
        namespace: Some("default".to_string()),
        yaml: "apiVersion: v1\nkind: Service\nmetadata:\n  name: api\n  namespace: default\nspec:\n  ports:\n    - port: 80\n"
            .to_string(),
        yaml_encoding: crate::models::YamlEncoding::default(),
        force_conflicts,
        expected_uid: None,
        expected_resource_version: None,
    }
}

fn reviewed_request_yaml(force_conflicts: bool) -> YamlApplyRequest {
    let mut request = base_request_yaml(force_conflicts);
    request.expected_uid = Some("uid-1".into());
    request.expected_resource_version = Some("7".into());
    request
}

fn assert_service_path(request: &Request<Body>) {
    assert!(
        request
            .uri()
            .path()
            .ends_with("/namespaces/default/services/api"),
        "unexpected path: {}",
        request.uri().path()
    );
}

fn assert_has_query_param(request: &Request<Body>, param: &str) {
    let query = request.uri().query().unwrap_or_default();
    assert!(query.contains(param), "query missing {param}: {query}");
}

fn assert_lacks_query_param(request: &Request<Body>, param: &str) {
    let query = request.uri().query().unwrap_or_default();
    assert!(
        !query.contains(param),
        "query unexpectedly has {param}: {query}"
    );
}

async fn await_mock<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("mock client response sequence timed out")
}

#[tokio::test]
async fn prepare_issues_get_then_dry_run_patch() {
    let (cluster, mut handle) = mock_cluster();
    let operation = cluster
        .scope()
        .run(prepare_yaml_apply(base_request_yaml(false)));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("get request");
        assert_eq!(request.method(), Method::GET);
        assert_service_path(&request);
        send.send_response(json_response(200, service_manifest()));

        let (request, send) = handle.next_request().await.expect("patch request");
        assert_eq!(request.method(), Method::PATCH);
        assert_service_path(&request);
        assert_has_query_param(&request, "dryRun=All");
        assert_has_query_param(&request, "fieldManager=kubecove");
        let patch: serde_json::Value =
            serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap()).unwrap();
        assert_eq!(patch["metadata"]["resourceVersion"], "7");
        send.send_response(json_response(200, service_manifest()));
    };

    let (preview, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    let preview = preview.unwrap();
    assert_eq!(cluster.intents(), vec![ClientIntent::Preview]);

    assert!(preview.current_yaml.contains("name: api"));
    assert!(preview.dry_run_yaml.contains("name: api"));
    assert_eq!(preview.uid, "uid-1");
    assert_eq!(preview.resource_version, "7");
}

#[tokio::test]
async fn apply_requires_reviewed_identity_before_cluster_access() {
    let (cluster, _handle) = mock_cluster();

    let error = cluster
        .scope()
        .run(apply_yaml(base_request_yaml(false)))
        .await
        .unwrap_err();

    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(error.message.contains("reviewed UID and resourceVersion"));
    assert_eq!(cluster.intents(), Vec::<ClientIntent>::new());
}

#[tokio::test]
async fn apply_rejects_a_resource_changed_after_review() {
    let (cluster, mut handle) = mock_cluster();
    let operation = cluster.scope().run(apply_yaml(reviewed_request_yaml(false)));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("identity get");
        assert_eq!(request.method(), Method::GET);
        assert_service_path(&request);
        let mut changed = service_manifest();
        changed["metadata"]["resourceVersion"] = json!("8");
        send.send_response(json_response(200, changed));
    };

    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    let error = result.unwrap_err();

    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(error.message.contains("changed since"));
    assert_eq!(cluster.intents(), vec![ClientIntent::ConfirmedOperation]);
}

#[tokio::test]
async fn apply_rechecks_identity_repeats_dry_run_and_guards_the_live_patch() {
    let (cluster, mut handle) = mock_cluster();
    let operation = cluster.scope().run(apply_yaml(reviewed_request_yaml(true)));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("identity get");
        assert_eq!(request.method(), Method::GET);
        assert_service_path(&request);
        send.send_response(json_response(200, service_manifest()));

        let (request, send) = handle.next_request().await.expect("repeated dry-run patch");
        assert_eq!(request.method(), Method::PATCH);
        assert_service_path(&request);
        assert_has_query_param(&request, "dryRun=All");
        assert_has_query_param(&request, "fieldManager=kubecove");
        assert_has_query_param(&request, "force=true");
        let patch: serde_json::Value =
            serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap()).unwrap();
        assert_eq!(patch["metadata"]["resourceVersion"], "7");
        send.send_response(json_response(200, service_manifest()));

        let (request, send) = handle.next_request().await.expect("guarded live patch");
        assert_eq!(request.method(), Method::PATCH);
        assert_service_path(&request);
        assert_has_query_param(&request, "fieldManager=kubecove");
        assert_has_query_param(&request, "force=true");
        assert_lacks_query_param(&request, "dryRun=");
        let patch: serde_json::Value =
            serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap()).unwrap();
        assert_eq!(patch["metadata"]["resourceVersion"], "7");
        send.send_response(json_response(200, service_manifest()));
    };

    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;

    assert!(result.unwrap().applied_yaml.contains("name: api"));
    // ADR 0009: workspace rotation must not cancel a write the API server may have applied.
    assert_eq!(cluster.intents(), vec![ClientIntent::ConfirmedOperation]);
}

#[tokio::test]
async fn apply_classifies_field_manager_conflict() {
    let (cluster, mut handle) = mock_cluster();
    let operation = cluster.scope().run(apply_yaml(reviewed_request_yaml(false)));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("identity get");
        assert_eq!(request.method(), Method::GET);
        assert_service_path(&request);
        send.send_response(json_response(200, service_manifest()));

        let (request, send) = handle.next_request().await.expect("dry-run patch");
        assert_eq!(request.method(), Method::PATCH);
        assert_service_path(&request);
        assert_has_query_param(&request, "dryRun=All");
        send.send_response(json_response(
            409,
            json!({
                "kind": "Status",
                "apiVersion": "v1",
                "metadata": {},
                "status": "Failure",
                "message": "Apply failed with conflicts",
                "reason": "Conflict",
                "details": {
                    "causes": [{
                        "reason": "FieldManagerConflict",
                        "message": "conflict with \"helm\"",
                        "field": ".spec.ports"
                    }]
                },
                "code": 409
            }),
        ));
    };

    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    let err = result.unwrap_err();

    assert_eq!(err.kind, AppErrorKind::FieldManagerConflict);
    assert_eq!(err.message, "Apply failed with conflicts");
}

#[tokio::test]
async fn apply_maps_forbidden_to_forbidden_kind() {
    let (cluster, mut handle) = mock_cluster();
    let operation = cluster.scope().run(apply_yaml(reviewed_request_yaml(false)));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("identity get");
        assert_eq!(request.method(), Method::GET);
        assert_service_path(&request);
        send.send_response(json_response(200, service_manifest()));

        let (request, send) = handle.next_request().await.expect("dry-run patch");
        assert_eq!(request.method(), Method::PATCH);
        assert_service_path(&request);
        assert_has_query_param(&request, "dryRun=All");
        send.send_response(json_response(
            403,
            json!({
                "kind": "Status",
                "apiVersion": "v1",
                "metadata": {},
                "status": "Failure",
                "message": "services \"api\" is forbidden",
                "reason": "Forbidden",
                "code": 403
            }),
        ));
    };

    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    let err = result.unwrap_err();

    assert_eq!(err.kind, AppErrorKind::Forbidden);
    assert!(err.message.contains("services \"api\" is forbidden"));
}
