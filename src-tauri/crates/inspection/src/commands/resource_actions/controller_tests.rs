use super::*;

fn owned_pod() -> Value {
    json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":"api","namespace":"default","uid":"pod-uid","resourceVersion":"7","labels":{"app":"api"},"ownerReferences":[{"apiVersion":"apps/v1","kind":"ReplicaSet","name":"api-rs","uid":"owner-uid","controller":true}]},"status":{"phase":"Running"}})
}

fn replica_set() -> Value {
    json!({"apiVersion":"apps/v1","kind":"ReplicaSet","metadata":{"name":"api-rs","namespace":"default","uid":"owner-uid"},"spec":{"replicas":1,"selector":{"matchLabels":{"app":"api"}}}})
}

async fn preview(
    pod: Value,
    owner: Option<Response<Body>>,
    pods: Option<Response<Body>>,
) -> crate::models::ResourceActionController {
    let (cluster, mut handle) = mock_cluster();
    let mut delete = request("Pod", "delete");
    delete.target.name = pod["metadata"]["name"].as_str().unwrap().to_owned();
    let operation = cluster.scope().run(preview_resource_action(delete));
    let responder = async move {
        let (_, send) = handle.next_request().await.expect("target read");
        send.send_response(json_response(200, pod.clone()));
        let (request, send) = handle.next_request().await.expect("delete dry-run");
        assert_eq!(request.method(), Method::DELETE);
        send.send_response(json_response(200, pod));
        if let Some(owner) = owner {
            let (request, send) = handle.next_request().await.expect("owner read");
            assert_eq!(request.method(), Method::GET);
            send.send_response(owner);
        }
        if let Some(pods) = pods {
            let (request, send) = handle.next_request().await.expect("Pod demand read");
            assert_eq!(request.method(), Method::GET);
            assert_eq!(request.uri().path(), "/api/v1/namespaces/default/pods");
            assert!(request.uri().query().unwrap().contains("limit=2"));
            assert!(
                request
                    .uri()
                    .query()
                    .unwrap()
                    .contains("labelSelector=app%3Dapi")
            );
            send.send_response(pods);
        }
        // Keep the responder alive: an unexpected extra read cannot pass.
        handle
    };
    let (result, _handle) =
        Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    result
        .expect("delete preview")
        .controller
        .expect("controlling owner")
}

fn pod_list(pods: Vec<Value>) -> Response<Body> {
    json_response(
        200,
        json!({"apiVersion":"v1","kind":"PodList","items":pods}),
    )
}

// Failure modes: live owner identity and lifecycle can be stale; Pod demand
// can exceed desired replicas, omit this target, or be unavailable through RBAC.
#[tokio::test]
async fn replica_preview_requires_live_demand_for_the_exact_pod() {
    let pod = owned_pod();
    assert!(
        preview(
            pod.clone(),
            Some(json_response(200, replica_set())),
            Some(pod_list(vec![pod.clone()]))
        )
        .await
        .recreates
    );
    let mut surplus = pod.clone();
    surplus["metadata"]["name"] = json!("surplus");
    surplus["metadata"]["uid"] = json!("surplus-uid");
    assert!(
        !preview(
            pod.clone(),
            Some(json_response(200, replica_set())),
            Some(pod_list(vec![pod.clone(), surplus.clone()]))
        )
        .await
        .recreates
    );
    assert!(
        !preview(
            pod.clone(),
            Some(json_response(200, replica_set())),
            Some(pod_list(vec![surplus]))
        )
        .await
        .recreates
    );
    assert!(
        !preview(
            pod.clone(),
            Some(json_response(200, replica_set())),
            Some(json_response(
                403,
                json!({"kind":"Status","status":"Failure","reason":"Forbidden","code":403})
            ))
        )
        .await
        .recreates
    );

    let mut terminating = replica_set();
    terminating["metadata"]["deletionTimestamp"] = json!("2026-11-13T19:00:00Z");
    let mut different_uid = replica_set();
    different_uid["metadata"]["uid"] = json!("different-owner");
    let mut scaled_zero = replica_set();
    scaled_zero["spec"]["replicas"] = json!(0);
    for response in [
        json_response(200, terminating),
        json_response(200, different_uid),
        json_response(200, scaled_zero),
        json_response(
            404,
            json!({"kind":"Status","status":"Failure","reason":"NotFound","code":404}),
        ),
        json_response(
            403,
            json!({"kind":"Status","status":"Failure","reason":"Forbidden","code":403}),
        ),
    ] {
        let controller = preview(pod.clone(), Some(response), None).await;
        assert_eq!(controller.name, "api-rs");
        assert!(!controller.recreates);
    }

    let mut mismatch = pod.clone();
    mismatch["metadata"]["labels"]["app"] = json!("other");
    assert!(
        !preview(mismatch, Some(json_response(200, replica_set())), None)
            .await
            .recreates
    );
}

// Terminal/deleting Pods, selector mismatches and unrelated owners do not
// count toward the controller's active selected population.
#[tokio::test]
async fn replica_demand_counts_only_active_owned_selected_pods() {
    let pod = owned_pod();
    let mut adoptable = pod.clone();
    adoptable["metadata"]["uid"] = json!("adoptable");
    adoptable["metadata"]
        .as_object_mut()
        .unwrap()
        .remove("ownerReferences");
    assert!(
        !preview(
            pod.clone(),
            Some(json_response(200, replica_set())),
            Some(pod_list(vec![pod.clone(), adoptable])),
        )
        .await
        .recreates
    );
    let mut terminal = pod.clone();
    terminal["metadata"]["uid"] = json!("finished");
    terminal["status"]["phase"] = json!("Succeeded");
    let mut deleting = pod.clone();
    deleting["metadata"]["uid"] = json!("deleting");
    deleting["metadata"]["deletionTimestamp"] = json!("2026-11-13T19:00:00Z");
    let mut unrelated = pod.clone();
    unrelated["metadata"]["uid"] = json!("unrelated");
    unrelated["metadata"]["ownerReferences"][0]["uid"] = json!("other-owner");
    assert!(
        preview(
            pod.clone(),
            Some(json_response(200, replica_set())),
            Some(pod_list(vec![pod, terminal, deleting, unrelated]))
        )
        .await
        .recreates
    );

    let mut rc_pod = owned_pod();
    rc_pod["metadata"]["ownerReferences"][0]["apiVersion"] = json!("v1");
    rc_pod["metadata"]["ownerReferences"][0]["kind"] = json!("ReplicationController");
    let rc = json!({"apiVersion":"v1","kind":"ReplicationController","metadata":{"name":"api-rs","namespace":"default","uid":"owner-uid"},"spec":{"replicas":1,"selector":{"app":"api"}}});
    assert!(
        preview(
            rc_pod.clone(),
            Some(json_response(200, rc)),
            Some(pod_list(vec![rc_pod]))
        )
        .await
        .recreates
    );
}

// A StatefulSet only replaces a canonical ordinal inside its desired interval.
#[tokio::test]
async fn statefulset_preview_requires_a_desired_pod_ordinal() {
    let mut owner = json!({"apiVersion":"apps/v1","kind":"StatefulSet","metadata":{"name":"web","namespace":"default","uid":"owner-uid"},"spec":{"replicas":1}});
    for (start, name, expected) in [
        (0, "web-0", true),
        (0, "web-2", false),
        (5, "web-4", false),
        (5, "web-5", true),
        (5, "web-6", false),
        (5, "other-5", false),
        (5, "web-no-index", false),
        (5, "web-05", false),
        (5, "web--5", false),
    ] {
        owner["spec"]["ordinals"] = json!({"start":start});
        let mut pod = owned_pod();
        pod["metadata"]["name"] = json!(name);
        pod["metadata"]["ownerReferences"][0]["kind"] = json!("StatefulSet");
        pod["metadata"]["ownerReferences"][0]["name"] = json!("web");
        assert_eq!(
            preview(pod, Some(json_response(200, owner.clone())), None)
                .await
                .recreates,
            expected,
            "{name}"
        );
    }
}

// Job retry/deadline state and DaemonSet node eligibility need more than
// owner evidence. They retain typed confirmation without claiming replacement.
#[tokio::test]
async fn job_and_daemonset_pods_keep_typed_confirmation() {
    for (api_version, kind) in [("batch/v1", "Job"), ("apps/v1", "DaemonSet")] {
        let mut pod = owned_pod();
        pod["metadata"]["ownerReferences"][0]["apiVersion"] = json!(api_version);
        pod["metadata"]["ownerReferences"][0]["kind"] = json!(kind);
        let controller = preview(pod, None, None).await;
        assert_eq!(controller.kind, kind);
        assert!(!controller.recreates);
    }
}

async fn execute_owned_pod(
    require_replacement: bool,
    owner_response: Option<Value>,
) -> Result<crate::models::ClusterOperationResult, crate::models::AppError> {
    let pod = owned_pod();
    let mut value = serde_json::to_value(request("Pod", "delete")).unwrap();
    value["confirmed"] = json!(true);
    value["expectedUid"] = json!("pod-uid");
    value["expectedResourceVersion"] = json!("7");
    value["requireReplacement"] = json!(require_replacement);
    let request = serde_json::from_value(value).unwrap();
    let (cluster, mut handle) = mock_cluster();
    let operation = cluster.scope().run(execute_resource_action(request));
    let responder = async move {
        let (_, send) = handle.next_request().await.expect("target read");
        send.send_response(json_response(200, pod.clone()));
        let (request, send) = handle.next_request().await.expect("delete dry-run");
        assert_eq!(request.method(), Method::DELETE);
        let body: Value =
            serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap()).unwrap();
        assert_eq!(body["dryRun"][0], "All");
        send.send_response(json_response(200, pod.clone()));
        if let Some(owner) = owner_response {
            let (request, send) = handle.next_request().await.expect("replacement recheck");
            assert_eq!(request.method(), Method::GET);
            assert!(request.uri().path().contains("/replicasets/"));
            let can_replace = owner["spec"]["replicas"] != 0;
            send.send_response(json_response(200, owner));
            if can_replace {
                let (request, send) = handle.next_request().await.expect("demand recheck");
                assert_eq!(request.uri().path(), "/api/v1/namespaces/default/pods");
                send.send_response(pod_list(vec![pod.clone()]));
            } else {
                return handle;
            }
        }
        let (request, send) = handle.next_request().await.expect("real delete");
        assert_eq!(request.method(), Method::DELETE);
        let body: Value =
            serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap()).unwrap();
        assert!(body.get("dryRun").is_none());
        assert_eq!(body["preconditions"]["uid"], "pod-uid");
        assert_eq!(body["preconditions"]["resourceVersion"], "7");
        send.send_response(json_response(200, pod));
        handle
    };
    let (result, _handle) =
        Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    result
}

// Replacement can disappear without changing the target's resourceVersion.
// A waived typed name requires fresh evidence; a typed delete needs no waiver.
#[tokio::test]
async fn waived_name_confirmation_rechecks_replacement_before_mutation() {
    let mut scaled_zero = replica_set();
    scaled_zero["spec"]["replicas"] = json!(0);
    let error = execute_owned_pod(true, Some(scaled_zero))
        .await
        .unwrap_err();
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(error.message.contains("Review"));
    assert!(execute_owned_pod(true, Some(replica_set())).await.is_ok());
    assert!(execute_owned_pod(false, None).await.is_ok());
}
