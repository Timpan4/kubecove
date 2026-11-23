use super::state::clean_action_display_metadata;
use super::*;
use crate::commands::{ClientIntent, FIXED_SOURCE_KEY, FixedClusterAccess};
use crate::models::{
    AppErrorKind, ResourceActionConfirmation, ResourceActionEdits, ResourceActionRequest,
};
use crate::runtime::State;
use http::{Method, Request, Response, header::CONTENT_TYPE};
use kube::{Client, client::Body};
use serde_json::{Value, json};
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

fn json_response(status: u16, value: Value) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&value).unwrap()))
        .unwrap()
}

async fn await_mock<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("mock request sequence timed out")
}

fn request(kind: &str, action: &str) -> ResourceActionRequest {
    serde_json::from_value(json!({
        "clusterContext": "kind-dev",
        "kind": kind,
        "name": "api",
        "namespace": "default",
        "action": action
    }))
    .expect("valid action request")
}

#[test]
fn execution_confirmation_rejects_caller_supplied_action_data() {
    let error = serde_json::from_value::<ResourceActionConfirmation>(json!({
        "reviewId": "review-1",
        "confirmation": "review-1",
        "action": "delete",
        "name": "other-target"
    }))
    .unwrap_err();
    assert!(error.to_string().contains("unknown field"));
}

#[tokio::test]
async fn execution_without_a_server_owned_review_is_rejected_before_cluster_access() {
    let reviews = ResourceActionReviewStore::default();
    let error = execute_resource_action(
        ResourceActionConfirmation {
            review_id: "missing-review".into(),
        },
        State::new(&reviews),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, AppErrorKind::ConfirmationRequired);
}

fn execution_request(action: &str) -> ResourceActionRequest {
    let mut request = request("ConfigMap", action);
    if action == "edit" {
        request.edits =
            Some(serde_json::from_value(json!({"labels":{"team":"platform"}})).unwrap());
    }
    request
}

fn configmap() -> Value {
    json!({"apiVersion":"v1","kind":"ConfigMap","metadata":{"name":"api","namespace":"default","uid":"uid-1","resourceVersion":"7","labels":{}},"data":{"key":"value"}})
}

fn deployment() -> Value {
    json!({"apiVersion":"apps/v1","kind":"Deployment","metadata":{"name":"api","namespace":"default","uid":"deployment-uid","resourceVersion":"7"},"spec":{"template":{"metadata":{"labels":{"app":"api"}},"spec":{"containers":[{"name":"api","image":"v2"}]}}}})
}

fn revision_set(uid: &str, resource_version: &str, image: &str) -> Value {
    json!({"apiVersion":"apps/v1","kind":"ReplicaSet","metadata":{"name":"api-abc","namespace":"default","uid":uid,"resourceVersion":resource_version,"annotations":{"deployment.kubernetes.io/revision":"2"},"ownerReferences":[{"apiVersion":"apps/v1","kind":"Deployment","name":"api","uid":"deployment-uid","controller":true}]},"spec":{"template":{"metadata":{"labels":{"app":"api","pod-template-hash":"abc"}},"spec":{"containers":[{"name":"api","image":image}]}}}})
}

fn rollback_request() -> ResourceActionRequest {
    let mut request = request("Deployment", "rollback");
    request.revision = Some(2);
    request
}

fn reviewed(
    request: ResourceActionRequest,
    uid: &str,
    resource_version: &str,
    revision_identity: Option<(&str, &str)>,
    patch: Option<Value>,
    reviewed_after: Option<Value>,
) -> (ResourceActionReviewStore, ResourceActionConfirmation) {
    let store = ResourceActionReviewStore::default();
    let review_id = store.begin().unwrap();
    store
        .complete(
            &review_id,
            ReviewedResourceAction {
                request,
                source_key: FIXED_SOURCE_KEY.into(),
                uid: uid.into(),
                resource_version: resource_version.into(),
                revision_identity: revision_identity
                    .map(|(uid, resource_version)| (uid.into(), resource_version.into())),
                patch,
                reviewed_after,
            },
        )
        .unwrap();
    (store, ResourceActionConfirmation { review_id })
}

fn assert_configmap_path(request: &Request<Body>) {
    assert_eq!(
        request.uri().path(),
        "/api/v1/namespaces/default/configmaps/api"
    );
}

fn merge_patch(target: &mut Value, patch: &Value) {
    if let Some(patch_object) = patch.as_object() {
        if !target.is_object() {
            *target = json!({});
        }
        let target_object = target.as_object_mut().expect("object initialized above");
        for (key, value) in patch_object {
            if value.is_null() {
                target_object.remove(key);
            } else {
                merge_patch(target_object.entry(key).or_insert_with(|| json!({})), value);
            }
        }
    } else {
        *target = patch.clone();
    }
}

#[test]
fn action_allowlist_rejects_secret_and_dynamic_targets() {
    for kind in ["Secret", "Widget"] {
        let error = validate_resource_action_request(&request(kind, "edit")).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::UnsupportedOperation);
    }
}

#[test]
fn action_allowlist_rejects_retargeted_api_versions() {
    let mut value = serde_json::to_value(request("Deployment", "restart")).unwrap();
    value["apiVersion"] = json!("v1");
    let request = serde_json::from_value(value).unwrap();
    assert_eq!(
        validate_resource_action_request(&request).unwrap_err().kind,
        AppErrorKind::UnsupportedOperation
    );
}

#[test]
fn permission_errors_fail_closed_and_keep_the_action_visible() {
    let denied = permission_from_review(
        ResourceActionKind::Edit,
        "patch",
        "deployments",
        Some(false),
        None,
    );
    let unknown = permission_from_review(
        ResourceActionKind::Restart,
        "patch",
        "deployments",
        None,
        Some("review failed"),
    );
    assert_eq!(unknown.action, ResourceActionKind::Restart);
    assert!(!denied.allowed);
    assert_eq!(denied.required_permission, "patch deployments");
    assert!(!unknown.allowed);
    assert!(unknown.reason.contains("review failed"));
}

#[test]
fn stale_resource_version_and_uid_block_execution() {
    assert_eq!(
        validate_review_guard("old-uid", "7", "current-uid", "7")
            .unwrap_err()
            .kind,
        AppErrorKind::Validation
    );
    assert_eq!(
        validate_review_guard("current-uid", "6", "current-uid", "7")
            .unwrap_err()
            .kind,
        AppErrorKind::Validation
    );
}

#[test]
fn env_edit_preserves_untouched_references_and_metadata() {
    let current = json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": "api", "namespace": "default", "labels": {"keep": "yes"},
            "annotations": {"note": "keep"}, "resourceVersion": "8", "uid": "uid-1"
        },
        "spec": {"replicas": 2, "selector": {"matchLabels": {"app": "api"}},
            "template": {"metadata": {"labels": {"app": "api"}}, "spec": {"containers": [
                {"name": "api", "image": "old", "env": [
                    {"name": "TOKEN", "valueFrom": {"secretKeyRef": {"name": "auth", "key": "token"}}},
                    {"name": "MODE", "value": "old"}
                ], "resources": {"requests": {"cpu": "10m"}, "limits": {"memory": "64Mi"}}}
            ]}}}
    });
    let edits: ResourceActionEdits = serde_json::from_value(json!({
        "containers": [{"name": "api", "env": [{"name": "MODE", "value": "new"}],
            "requests": {"cpu": "20m"}}],
        "labels": {"new": "yes"}
    }))
    .unwrap();
    let patch =
        build_resource_action_patch(&request("Deployment", "edit"), &edits, &current).unwrap();
    let mut after = current.clone();
    merge_patch(&mut after, &patch);
    assert_eq!(after["metadata"]["labels"]["new"], "yes");
    assert_eq!(after["metadata"]["resourceVersion"], "8");
    assert_eq!(after["spec"]["replicas"], 2);
    assert_eq!(
        after["spec"]["template"]["metadata"]["labels"]["app"],
        "api"
    );
    assert_eq!(
        after["spec"]["template"]["spec"]["containers"][0]["env"][0]["valueFrom"]["secretKeyRef"]["name"],
        "auth"
    );
    assert_eq!(
        after["spec"]["template"]["spec"]["containers"][0]["env"][1]["value"],
        "new"
    );
    assert_eq!(
        after["spec"]["template"]["spec"]["containers"][0]["resources"]["requests"]["cpu"],
        "20m"
    );
    assert_eq!(
        after["spec"]["template"]["spec"]["containers"][0]["resources"]["limits"]["memory"],
        "64Mi"
    );
}

#[test]
fn first_env_edit_adds_a_variable_when_the_container_has_no_env_field() {
    let current = json!({"kind":"Deployment","metadata":{"uid":"uid-1","resourceVersion":"8"},
        "spec":{"template":{"spec":{"containers":[{"name":"api","image":"v1"}]}}}});
    let edits: ResourceActionEdits = serde_json::from_value(
        json!({"containers":[{"name":"api","env":[{"name":"MODE","value":"safe"}]}]}),
    )
    .unwrap();
    let patch =
        build_resource_action_patch(&request("Deployment", "edit"), &edits, &current).unwrap();
    let mut after = current;
    merge_patch(&mut after, &patch);
    assert_eq!(
        after["spec"]["template"]["spec"]["containers"][0]["env"][0]["name"],
        "MODE"
    );
    assert_eq!(
        after["spec"]["template"]["spec"]["containers"][0]["env"][0]["value"],
        "safe"
    );
}

#[test]
fn metadata_delta_null_removes_only_the_named_key_under_merge_patch_semantics() {
    let current = json!({"kind":"ConfigMap","metadata":{"uid":"uid-1","resourceVersion":"8",
        "labels":{"keep":"yes","remove":"old"},"annotations":{"keep":"yes","remove":"old"}}});
    let edits: ResourceActionEdits =
        serde_json::from_value(json!({"labels":{"add":"new","remove":null},
        "annotations":{"add":"new","remove":null}}))
        .unwrap();
    let patch =
        build_resource_action_patch(&request("ConfigMap", "edit"), &edits, &current).unwrap();
    let mut after = current;
    merge_patch(&mut after, &patch);
    assert_eq!(
        after["metadata"]["labels"],
        json!({"keep":"yes","add":"new"})
    );
    assert_eq!(
        after["metadata"]["annotations"],
        json!({"keep":"yes","add":"new"})
    );
}

#[test]
fn rollback_requires_an_owned_revision_and_restores_the_complete_template() {
    let owner =
        json!({"kind": "Deployment", "name": "api", "uid": "owner-uid", "controller": true});
    let revision = json!({
        "metadata": {"annotations": {"deployment.kubernetes.io/revision": "3"}, "ownerReferences": [owner]},
        "spec": {"template": {"metadata": {"labels": {"app": "api", "pod-template-hash": "abc123"}, "annotations": {"custom": "preserve"}},
            "spec": {"containers": [{"name": "api", "image": "v3", "env": [{"name": "A", "value": "1"}]}], "volumes": [{"name": "data", "emptyDir": {}}]}}}
    });
    let template = owned_revision_template(&revision, "api", "owner-uid").unwrap();
    assert_eq!(template["metadata"]["annotations"]["custom"], "preserve");
    assert_eq!(template["metadata"]["labels"]["app"], "api");
    assert!(
        template["metadata"]["labels"]
            .get("pod-template-hash")
            .is_none()
    );
    assert_eq!(template["spec"]["volumes"][0]["name"], "data");
    assert_eq!(template["spec"]["containers"][0]["image"], "v3");
    assert!(owned_revision_template(&revision, "api", "different-uid").is_err());
}

#[test]
fn rollback_patch_removes_template_fields_added_after_the_revision() {
    let before = json!({"metadata":{"labels":{"new":"label"}},"spec":{"containers":[{"name":"api","image":"v4","env":[{"name":"NEW","value":"later"}]}],"volumes":[{"name":"new-volume","emptyDir":{}}]}});
    let revision = json!({"metadata":{"labels":{"old":"label"}},"spec":{"containers":[{"name":"api","image":"v3"}]}});
    let patch = complete_template_merge_patch(&before, &revision);
    let mut actual = before.clone();
    merge_patch(&mut actual, &patch);
    assert_eq!(actual, revision);
}

#[test]
fn edit_patch_cleans_server_metadata_and_preview_never_mutates_without_dry_run() {
    let mut current = json!({"metadata": {"name":"api", "namespace":"default", "uid":"uid", "resourceVersion":"4",
        "managedFields":[{"manager":"server"}], "creationTimestamp":"now", "labels":{"keep":"yes"}},
        "spec":{"replicas":1}});
    let patch = build_resource_action_patch(
        &request("Deployment", "edit"),
        &ResourceActionEdits {
            replicas: Some(3),
            ..Default::default()
        },
        &current,
    )
    .unwrap();
    assert!(patch["metadata"].get("managedFields").is_none());
    assert_eq!(patch["metadata"]["resourceVersion"], "4");
    clean_action_display_metadata(&mut current);
    assert!(current["metadata"].get("managedFields").is_none());
    assert!(preview_patch_params().dry_run);
}

#[test]
fn deletion_exact_preconditions_are_mandatory() {
    let params = exact_delete_params("uid-1", "12");
    assert_eq!(params.preconditions.unwrap().uid.as_deref(), Some("uid-1"));
    let dry_run = preview_exact_delete_params("uid-1", "12");
    assert!(dry_run.dry_run);
    assert_eq!(
        dry_run.preconditions.unwrap().resource_version.as_deref(),
        Some("12")
    );
}

#[tokio::test]
async fn execute_edit_repeats_dry_run_before_guarded_patch() {
    let (cluster, mut handle) = mock_cluster();
    let mut reviewed_after = configmap();
    reviewed_after["metadata"]["labels"]["team"] = json!("platform");
    let (reviews, confirmation) = reviewed(
        execution_request("edit"),
        "uid-1",
        "7",
        None,
        Some(
            json!({"metadata":{"labels":{"team":"platform"},"uid":"uid-1","resourceVersion":"7"}}),
        ),
        Some(state::action_review_value(&reviewed_after)),
    );
    let operation = cluster
        .scope()
        .run(execute_resource_action(confirmation, State::new(&reviews)));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("target read");
        assert_eq!(request.method(), Method::GET);
        assert_configmap_path(&request);
        send.send_response(json_response(200, configmap()));

        let (request, send) = handle.next_request().await.expect("mandatory dry-run");
        assert_eq!(request.method(), Method::PATCH);
        assert_configmap_path(&request);
        assert!(
            request
                .uri()
                .query()
                .unwrap_or_default()
                .contains("dryRun=All")
        );
        send.send_response(json_response(200, reviewed_after.clone()));

        let (request, send) = handle.next_request().await.expect("guarded apply");
        assert_eq!(request.method(), Method::PATCH);
        assert_configmap_path(&request);
        assert!(
            !request
                .uri()
                .query()
                .unwrap_or_default()
                .contains("dryRun=")
        );
        send.send_response(json_response(200, reviewed_after));
    };
    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    assert!(result.is_ok());
    // ADR 0009: workspace rotation must not cancel a write the API server may have applied.
    assert_eq!(cluster.intents(), vec![ClientIntent::ConfirmedOperation]);
}

#[tokio::test]
async fn failed_execution_dry_run_never_sends_mutating_patch() {
    let (cluster, mut handle) = mock_cluster();
    let (reviews, confirmation) = reviewed(
        execution_request("edit"),
        "uid-1",
        "7",
        None,
        Some(
            json!({"metadata":{"labels":{"team":"platform"},"uid":"uid-1","resourceVersion":"7"}}),
        ),
        None,
    );
    let operation = cluster.scope().run(execute_resource_action(
        confirmation.clone(),
        State::new(&reviews),
    ));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("target read");
        assert_eq!(request.method(), Method::GET);
        send.send_response(json_response(200, configmap()));
        let (request, send) = handle.next_request().await.expect("dry-run patch");
        assert_eq!(request.method(), Method::PATCH);
        assert!(
            request
                .uri()
                .query()
                .unwrap_or_default()
                .contains("dryRun=All")
        );
        send.send_response(json_response(422, json!({"kind":"Status","status":"Failure","reason":"Invalid","message":"dry-run rejected","code":422})));
    };
    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    assert!(result.is_err());
    assert_eq!(
        execute_resource_action(confirmation, State::new(&reviews))
            .await
            .unwrap_err()
            .kind,
        AppErrorKind::ConfirmationRequired
    );
}

#[tokio::test]
async fn execute_delete_dry_runs_then_uses_uid_and_resource_version_preconditions() {
    let (cluster, mut handle) = mock_cluster();
    let (reviews, confirmation) =
        reviewed(execution_request("delete"), "uid-1", "7", None, None, None);
    let operation = cluster
        .scope()
        .run(execute_resource_action(confirmation, State::new(&reviews)));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("target read");
        assert_eq!(request.method(), Method::GET);
        send.send_response(json_response(200, configmap()));

        let (request, send) = handle.next_request().await.expect("delete dry-run");
        assert_eq!(request.method(), Method::DELETE);
        let body: Value =
            serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap()).unwrap();
        assert_eq!(body["dryRun"][0], "All");
        assert_eq!(body["preconditions"]["uid"], "uid-1");
        assert_eq!(body["preconditions"]["resourceVersion"], "7");
        send.send_response(json_response(
            200,
            json!({"kind":"Status","status":"Success","code":200}),
        ));

        let (request, send) = handle.next_request().await.expect("real guarded delete");
        assert_eq!(request.method(), Method::DELETE);
        let body: Value =
            serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap()).unwrap();
        assert!(body.get("dryRun").is_none());
        assert_eq!(body["preconditions"]["uid"], "uid-1");
        assert_eq!(body["preconditions"]["resourceVersion"], "7");
        send.send_response(json_response(
            200,
            json!({"kind":"Status","status":"Success","code":200}),
        ));
    };
    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    assert!(result.is_ok());
    assert_eq!(cluster.intents(), vec![ClientIntent::ConfirmedOperation]);
}

#[tokio::test]
async fn rollback_execution_rejects_a_changed_revision_source_before_patch() {
    let (cluster, mut handle) = mock_cluster();
    let (reviews, confirmation) = reviewed(
        rollback_request(),
        "deployment-uid",
        "7",
        Some(("revision-uid", "10")),
        Some(json!({"metadata":{"uid":"deployment-uid","resourceVersion":"7"}})),
        None,
    );
    let operation = cluster
        .scope()
        .run(execute_resource_action(confirmation, State::new(&reviews)));
    let responder = async move {
        let mut operation = Box::pin(operation);
        loop {
            tokio::select! {
                result = &mut operation => break result,
                request = handle.next_request() => {
                    let (request, send) = request.expect("mock request stream");
                    assert_eq!(request.method(), Method::GET);
                    if request.uri().path() == "/apis/apps/v1/namespaces/default/deployments/api" {
                        send.send_response(json_response(200, deployment()));
                    } else {
                        assert_eq!(request.uri().path(), "/apis/apps/v1/namespaces/default/replicasets");
                        send.send_response(json_response(200, json!({"apiVersion":"apps/v1","kind":"ReplicaSetList","items":[revision_set("revision-uid", "11", "v1")]})));
                    }
                }
            }
        }
    };
    let result = Box::pin(await_mock(responder)).await;
    assert_eq!(result.unwrap_err().kind, AppErrorKind::Validation);
}

#[tokio::test]
async fn rollback_preview_returns_identity_of_the_revision_used_for_its_dry_run_patch() {
    let (cluster, mut handle) = mock_cluster();
    let reviews = ResourceActionReviewStore::default();
    let operation = cluster.scope().run(preview_resource_action(
        rollback_request(),
        State::new(&reviews),
    ));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("target read");
        assert_eq!(request.method(), Method::GET);
        send.send_response(json_response(200, deployment()));

        let (request, send) = handle.next_request().await.expect("owned revision lookup");
        assert_eq!(request.method(), Method::GET);
        assert_eq!(
            request.uri().path(),
            "/apis/apps/v1/namespaces/default/replicasets"
        );
        send.send_response(json_response(200, json!({"apiVersion":"apps/v1","kind":"ReplicaSetList","items":[revision_set("reviewed-revision-uid", "42", "v1")]})));

        let (request, send) = handle.next_request().await.expect("rollback dry-run patch");
        assert_eq!(request.method(), Method::PATCH);
        assert!(
            request
                .uri()
                .query()
                .unwrap_or_default()
                .contains("dryRun=All")
        );
        let patch: Value =
            serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap()).unwrap();
        assert_eq!(
            patch["spec"]["template"]["spec"]["containers"][0]["image"],
            "v1"
        );
        let mut after = deployment();
        after["metadata"]["resourceVersion"] = json!("7");
        after["spec"]["template"] =
            revision_set("reviewed-revision-uid", "42", "v1")["spec"]["template"].clone();
        send.send_response(json_response(200, after));
    };
    let (preview, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    let preview = preview.unwrap();
    assert_eq!(cluster.intents(), vec![ClientIntent::Preview]);
    assert_eq!(
        preview.revision_uid.as_deref(),
        Some("reviewed-revision-uid")
    );
    assert_eq!(preview.revision_resource_version.as_deref(), Some("42"));
    assert!(preview.after_yaml.contains("image: v1"));
}

#[test]
fn secret_objects_are_rejected_before_any_yaml_is_returned() {
    let mut value: Value = json!({"kind":"Secret", "data":{"token":"dG9rZW4="}});
    assert!(ensure_action_resource_is_not_secret(&value).is_err());
    value["kind"] = json!("ConfigMap");
    assert!(ensure_action_resource_is_not_secret(&value).is_ok());
}
#[path = "resource_actions/controller_tests.rs"]
mod controller_tests;
#[path = "resource_actions/review_binding_tests.rs"]
mod review_binding_tests;
