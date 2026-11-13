use super::*;

fn owned_pod() -> Value {
    json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":"api","namespace":"default","uid":"pod-uid","resourceVersion":"7","ownerReferences":[{"apiVersion":"apps/v1","kind":"ReplicaSet","name":"api-7d9c9b7f8d","uid":"rs-uid","controller":true}]},"status":{"phase":"Running"}})
}

/// Previews a Pod delete and answers the owner read with `owner`.
async fn preview_pod_delete(
    owner: Response<Body>,
) -> Option<crate::models::ResourceActionController> {
    preview_owned_pod_delete(
        owned_pod(),
        owner,
        "/apis/apps/v1/namespaces/default/replicasets/api-7d9c9b7f8d",
    )
    .await
}

async fn preview_owned_pod_delete(
    pod: Value,
    owner: Response<Body>,
    owner_path: &str,
) -> Option<crate::models::ResourceActionController> {
    let (cluster, mut handle) = mock_cluster();
    let mut delete = request("Pod", "delete");
    delete.target.name = pod["metadata"]["name"].as_str().unwrap().to_owned();
    let operation = cluster.scope().run(preview_resource_action(delete));
    let responder = async move {
        let (_, send) = handle.next_request().await.expect("target read");
        send.send_response(json_response(200, pod.clone()));
        let (_, send) = handle.next_request().await.expect("delete dry-run");
        send.send_response(json_response(200, pod));
        let (request, send) = handle.next_request().await.expect("owner read");
        assert_eq!(request.method(), Method::GET);
        assert_eq!(request.uri().path(), owner_path);
        send.send_response(owner);
    };
    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    result.expect("delete preview").controller
}

async fn preview_controller_pod(pod_name: &str, owner: Value, path: &str) -> bool {
    let mut pod = owned_pod();
    pod["metadata"]["name"] = json!(pod_name);
    pod["metadata"]["ownerReferences"][0] = json!({
        "apiVersion": owner["apiVersion"],
        "kind": owner["kind"],
        "name": owner["metadata"]["name"],
        "uid": owner["metadata"]["uid"],
        "controller": true,
    });
    preview_owned_pod_delete(pod, json_response(200, owner), path)
        .await
        .expect("controller")
        .recreates
}

// Failure modes: positive replicas can exclude a Pod after scale-down or a
// changed start ordinal. A malformed name cannot establish a desired ordinal.
#[tokio::test]
async fn statefulset_preview_requires_a_desired_pod_ordinal() {
    let mut owner = json!({"apiVersion":"apps/v1","kind":"StatefulSet","metadata":{"name":"web","namespace":"default","uid":"ss-uid"},"spec":{"replicas":1}});
    let path = "/apis/apps/v1/namespaces/default/statefulsets/web";
    assert!(preview_controller_pod("web-0", owner.clone(), path).await);
    assert!(!preview_controller_pod("web-2", owner.clone(), path).await);
    owner["spec"]["ordinals"] = json!({"start":5});
    assert!(!preview_controller_pod("web-4", owner.clone(), path).await);
    assert!(preview_controller_pod("web-5", owner.clone(), path).await);
    assert!(!preview_controller_pod("web-6", owner.clone(), path).await);
    for name in ["other-5", "web-no-index", "web-05", "web--5"] {
        assert!(
            !preview_controller_pod(name, owner.clone(), path).await,
            "{name}"
        );
    }
}

// Failure modes: Job termination conditions precede terminal Pod phases;
// paused, completed, externally managed or indexed work may not be replaced.
#[tokio::test]
async fn job_preview_requires_active_replacement_work() {
    let owner = json!({"apiVersion":"batch/v1","kind":"Job","metadata":{"name":"batch","namespace":"default","uid":"job-uid"},"spec":{"parallelism":1,"completions":1}});
    let path = "/apis/batch/v1/namespaces/default/jobs/batch";
    assert!(preview_controller_pod("batch-abc", owner.clone(), path).await);
    for condition in ["FailureTarget", "Failed", "SuccessCriteriaMet", "Complete"] {
        let mut inactive = owner.clone();
        inactive["status"] = json!({"conditions":[{"type":condition,"status":"True"}]});
        assert!(
            !preview_controller_pod("batch-abc", inactive.clone(), path).await,
            "{condition}"
        );
        inactive["status"]["conditions"][0]["status"] = json!("False");
        assert!(
            preview_controller_pod("batch-abc", inactive, path).await,
            "{condition}=False"
        );
    }
    for (field, value) in [
        ("parallelism", json!(0)),
        ("suspend", json!(true)),
        ("managedBy", json!("example.com/job-controller")),
        ("completionMode", json!("Indexed")),
    ] {
        let mut inactive = owner.clone();
        inactive["spec"][field] = value;
        assert!(
            !preview_controller_pod("batch-abc", inactive, path).await,
            "{field}"
        );
    }
    let mut completed = owner.clone();
    completed["status"] = json!({"succeeded":1});
    assert!(!preview_controller_pod("batch-abc", completed, path).await);
    let mut work_queue = owner;
    work_queue["spec"]
        .as_object_mut()
        .unwrap()
        .remove("completions");
    work_queue["status"] = json!({"succeeded":1});
    assert!(!preview_controller_pod("batch-abc", work_queue, path).await);
}

// Failure modes, written before the code: the owner read can find the owner
// active, find it terminating, find nothing (deleted with orphaning), or be
// refused by RBAC. Only the first may skip the typed name.
#[tokio::test]
async fn pod_delete_preview_trusts_only_a_live_active_owner() {
    let replica_set = |metadata: Value| json!({"apiVersion":"apps/v1","kind":"ReplicaSet","metadata":metadata,"spec":{"replicas":2}});
    let live = replica_set(json!({"name":"api-7d9c9b7f8d","namespace":"default","uid":"rs-uid"}));
    let controller = preview_pod_delete(json_response(200, live))
        .await
        .expect("controller");
    assert_eq!(
        (controller.name.as_str(), controller.recreates),
        ("api-7d9c9b7f8d", true)
    );

    let terminating = replica_set(
        json!({"name":"api-7d9c9b7f8d","namespace":"default","uid":"rs-uid","deletionTimestamp":"2026-11-13T19:00:00Z"}),
    );
    for owner in [
        json_response(200, terminating),
        json_response(
            404,
            json!({"kind":"Status","status":"Failure","reason":"NotFound","code":404}),
        ),
        json_response(
            403,
            json!({"kind":"Status","status":"Failure","reason":"Forbidden","code":403}),
        ),
    ] {
        let controller = preview_pod_delete(owner).await.expect("controller");
        assert!(!controller.recreates);
    }
}
