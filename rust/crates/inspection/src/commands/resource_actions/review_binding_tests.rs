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
        let mut repeated = deployment();
        merge_patch(&mut repeated, &reviewed_patch);
        repeated["metadata"]["resourceVersion"] = json!("8");
        repeated["metadata"]["generation"] = json!(3);
        repeated["metadata"]["managedFields"] = json!([{"manager": "kubecove"}]);
        repeated["status"] = json!({"observedGeneration": 2});
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
            let response = if expected_dry_run {
                repeated.clone()
            } else {
                deployment()
            };
            send.send_response(json_response(200, response));
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

async fn preview_answered_with(
    cluster: &Arc<FixedClusterAccess>,
    mut handle: MockHandle,
    reviews: &ResourceActionReviewStore,
    request: ResourceActionRequest,
    dry_run: Value,
) -> (ResourceActionConfirmation, MockHandle) {
    let operation = cluster
        .scope()
        .run(preview_resource_action(request, State::new(reviews)));
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("preview target read");
        assert_eq!(request.method(), Method::GET);
        send.send_response(json_response(200, configmap()));
        let (request, send) = handle.next_request().await.expect("preview dry run");
        assert_eq!(request.method(), Method::PATCH);
        send.send_response(json_response(200, dry_run));
        handle
    };
    let (preview, handle) =
        Box::pin(await_mock(async { tokio::join!(operation, responder) })).await;
    let confirmation = ResourceActionConfirmation {
        review_id: preview.unwrap().review_id,
    };
    (confirmation, handle)
}

async fn execute_answered_with(
    cluster: &Arc<FixedClusterAccess>,
    mut handle: MockHandle,
    reviews: &ResourceActionReviewStore,
    confirmation: ResourceActionConfirmation,
    dry_run: Value,
) -> Result<ClusterOperationResult, AppError> {
    let mut operation = Box::pin(
        cluster
            .scope()
            .run(execute_resource_action(confirmation, State::new(reviews))),
    );
    let mut dry_run = Some(dry_run);
    Box::pin(await_mock(async {
        loop {
            tokio::select! {
                result = &mut operation => break result,
                request = handle.next_request() => {
                    let (request, send) = request.expect("mock request stream");
                    if request.method() == Method::GET {
                        send.send_response(json_response(200, configmap()));
                        continue;
                    }
                    assert_eq!(request.method(), Method::PATCH);
                    assert!(
                        request.uri().query().unwrap_or_default().contains("dryRun=All"),
                        "a real patch was sent after a changed dry-run result"
                    );
                    send.send_response(json_response(200, dry_run.take().expect("one dry run")));
                }
            }
        }
    }))
    .await
}

#[tokio::test]
async fn an_edit_whose_repeated_dry_run_result_changed_is_refused_before_the_real_patch() {
    let (cluster, handle) = mock_cluster();
    let reviews = ResourceActionReviewStore::default();
    let mut reviewed_after = configmap();
    reviewed_after["metadata"]["labels"] = json!({"team": "platform"});
    let (confirmation, handle) = preview_answered_with(
        &cluster,
        handle,
        &reviews,
        execution_request("edit"),
        reviewed_after.clone(),
    )
    .await;

    let mut changed_after = reviewed_after;
    changed_after["data"]["key"] = json!("changed by an admission webhook");
    let error = execute_answered_with(
        &cluster,
        handle,
        &reviews,
        confirmation.clone(),
        changed_after,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(error.message.contains("dry-run result changed"));
    assert_eq!(
        execute_resource_action(confirmation, State::new(&reviews))
            .await
            .unwrap_err()
            .kind,
        AppErrorKind::ConfirmationRequired
    );
}
