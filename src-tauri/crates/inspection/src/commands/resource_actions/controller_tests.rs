use super::*;

// Owner metadata must survive old revisions and terminal Pods, without owner
// or population reads. Missing, non-controlling and malformed owners show none.
#[tokio::test]
async fn delete_preview_displays_owner_metadata_without_replacement_queries() {
    for (owners, expected) in [
        (json!([]), None),
        (
            json!([{"kind":"ReplicaSet","name":"old-rs","controller":false}]),
            None,
        ),
        (json!([{"kind":"ReplicaSet","controller":true}]), None),
        (
            json!([{"kind":"","name":"invalid","controller":true}]),
            None,
        ),
        (
            json!([
                {"kind":"ConfigMap","name":"reference"},
                {"kind":"ReplicaSet","name":"old-rs","controller":true}
            ]),
            Some(("ReplicaSet", "old-rs")),
        ),
        (
            json!([{"kind":"Rollout","name":"custom","controller":true}]),
            Some(("Rollout", "custom")),
        ),
    ] {
        let pod = json!({
            "apiVersion":"v1","kind":"Pod",
            "metadata":{"name":"api","namespace":"default","uid":"pod-uid","resourceVersion":"7","ownerReferences":owners},
            "status":{"phase":"Succeeded"}
        });
        let (cluster, mut handle) = mock_cluster();
        let operation = cluster
            .scope()
            .run(preview_resource_action(request("Pod", "delete")));
        let responder = async move {
            let (request, send) = handle.next_request().await.expect("target read");
            assert_eq!(request.method(), Method::GET);
            send.send_response(json_response(200, pod.clone()));
            let (request, send) = handle.next_request().await.expect("delete dry-run");
            assert_eq!(request.method(), Method::DELETE);
            let body: Value =
                serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap())
                    .unwrap();
            assert_eq!(body["dryRun"][0], "All");
            send.send_response(json_response(200, pod));
            // Any further request remains unanswered and fails await_mock.
            handle
        };
        let (result, _handle) =
            Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
        let controller = result.expect("delete preview").controller;
        assert_eq!(
            controller
                .as_ref()
                .map(|owner| (owner.kind.as_str(), owner.name.as_str())),
            expected
        );
    }
}
