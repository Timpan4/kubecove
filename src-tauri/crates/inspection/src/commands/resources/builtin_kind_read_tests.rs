use super::{
    details::resource_details_from, summary::resources_summary_from, yaml::resource_yaml_from,
};
use crate::commands::{builtin_kinds::BUILTIN_KINDS, ClusterAccess, FixedClusterAccess};
use crate::models::{AppError, AppErrorKind};
use http::{header::CONTENT_TYPE, Request, Response};
use kube::client::Body;
use std::future::Future;

/// Answers every request with 404 until the client is dropped, so a read path
/// that reaches the API reports `NotFound` instead of an unsupported kind.
async fn with_missing_objects<T, F, Fut>(read: F) -> Result<T, AppError>
where
    F: FnOnce(ClusterAccess) -> Fut,
    Fut: Future<Output = Result<T, AppError>>,
{
    let (service, mut handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    let responder = async move {
        while let Some((_, send)) = handle.next_request().await {
            let status = serde_json::json!({
                "apiVersion": "v1", "kind": "Status", "metadata": {}, "status": "Failure",
                "message": "not found", "reason": "NotFound", "code": 404
            });
            send.send_response(
                Response::builder()
                    .status(404)
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&status).unwrap()))
                    .unwrap(),
            );
        }
    };
    let access = FixedClusterAccess::new(kube::Client::new(service, "default")).access();
    let (result, ()) = tokio::join!(read(access), responder);
    result
}

#[tokio::test]
async fn every_table_kind_is_served_by_summary_details_and_yaml_reads() {
    for entry in BUILTIN_KINDS.iter().filter(|entry| entry.table) {
        let namespace = entry.namespaced.then(|| "default".to_string());
        let reads = [
            (
                "summary",
                with_missing_objects(|access| {
                    let namespace = namespace.clone();
                    async move {
                        resources_summary_from(
                            &access,
                            "kind-dev".into(),
                            entry.kind.into(),
                            namespace,
                        )
                        .await
                    }
                })
                .await
                .map(drop),
            ),
            (
                "details",
                with_missing_objects(|access| {
                    let namespace = namespace.clone();
                    async move {
                        resource_details_from(
                            &access,
                            "kind-dev".into(),
                            entry.kind.into(),
                            "missing".into(),
                            namespace,
                        )
                        .await
                    }
                })
                .await
                .map(drop),
            ),
            (
                "yaml",
                with_missing_objects(|access| {
                    let namespace = namespace.clone();
                    async move {
                        resource_yaml_from(
                            &access,
                            "kind-dev".into(),
                            entry.kind.into(),
                            "missing".into(),
                            namespace,
                            None,
                            None,
                        )
                        .await
                    }
                })
                .await
                .map(drop),
            ),
        ];
        for (path, result) in reads {
            let error = result.expect_err("every object is missing");
            assert_eq!(
                error.kind,
                AppErrorKind::NotFound,
                "{path} read for {} did not reach the API: {}",
                entry.kind,
                error.message
            );
        }
    }
}
