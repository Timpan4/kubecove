use super::*;

#[tokio::test]
async fn restart_execution_reuses_the_exact_reviewed_patch_and_cannot_be_replayed() {
    let (cluster, mut handle) = mock_cluster();
    let reviews = ResourceActionReviewStore::default();
    let preview_operation = cluster.scope().run(preview_resource_action(
        request("Deployment", "restart"),
        State::new(&reviews),
    ));
    let preview_responder = async move {
        let (request, send) = handle.next_request().await.expect("preview target read");
        assert_eq!(request.method(), Method::GET);
        send.send_response(json_response(200, deployment()));
        let (request, send) = handle.next_request().await.expect("preview dry run");
        assert_eq!(request.method(), Method::PATCH);
        let reviewed_patch: Value =
            serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap()).unwrap();
        let mut after = deployment();
        merge_patch(&mut after, &reviewed_patch);
        send.send_response(json_response(200, after));
        (handle, reviewed_patch)
    };
    let (preview, (next_handle, reviewed_patch)) = Box::pin(await_mock(async {
        tokio::join!(preview_operation, preview_responder)
    }))
    .await;
    let preview = preview.unwrap();
    handle = next_handle;

    let confirmation = ResourceActionConfirmation {
        review_id: preview.review_id,
    };
    let operation = cluster.scope().run(execute_resource_action(
        confirmation.clone(),
        State::new(&reviews),
    ));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("execution target read");
        assert_eq!(request.method(), Method::GET);
        send.send_response(json_response(200, deployment()));
        for expected_dry_run in [true, false] {
            let (request, send) = handle.next_request().await.expect("reviewed patch");
            assert_eq!(request.method(), Method::PATCH);
            assert_eq!(
                request
                    .uri()
                    .query()
                    .unwrap_or_default()
                    .contains("dryRun=All"),
                expected_dry_run
            );
            let patch: Value =
                serde_json::from_slice(&request.into_body().collect_bytes().await.unwrap())
                    .unwrap();
            assert_eq!(patch, reviewed_patch);
            send.send_response(json_response(200, deployment()));
        }
    };
    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    assert!(result.is_ok());
    assert_eq!(
        execute_resource_action(confirmation, State::new(&reviews))
            .await
            .unwrap_err()
            .kind,
        AppErrorKind::ConfirmationRequired
    );
}

#[tokio::test]
async fn a_target_changed_after_preview_is_rejected_before_any_patch() {
    let (cluster, mut handle) = mock_cluster();
    let reviews = ResourceActionReviewStore::default();
    let preview_operation = cluster.scope().run(preview_resource_action(
        execution_request("edit"),
        State::new(&reviews),
    ));
    let preview_responder = async move {
        let (request, send) = handle.next_request().await.expect("preview target read");
        assert_eq!(request.method(), Method::GET);
        send.send_response(json_response(200, configmap()));
        let (request, send) = handle.next_request().await.expect("preview dry run");
        assert_eq!(request.method(), Method::PATCH);
        send.send_response(json_response(200, configmap()));
        handle
    };
    let (preview, next_handle) = Box::pin(await_mock(async {
        tokio::join!(preview_operation, preview_responder)
    }))
    .await;
    handle = next_handle;
    let operation = cluster.scope().run(execute_resource_action(
        ResourceActionConfirmation {
            review_id: preview.unwrap().review_id,
        },
        State::new(&reviews),
    ));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("execution target read");
        assert_eq!(request.method(), Method::GET);
        let mut changed = configmap();
        changed["metadata"]["resourceVersion"] = json!("8");
        send.send_response(json_response(200, changed));
    };
    let (result, ()) = Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    assert_eq!(result.unwrap_err().kind, AppErrorKind::Validation);
}
