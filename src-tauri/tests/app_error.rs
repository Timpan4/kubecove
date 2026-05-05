use k8s_manager_lib::models::AppError;
use serde_json::json;

#[test]
fn app_error_serializes_user_visible_kind_and_message() {
    let error = AppError::Cluster("context kind-dev is unavailable".to_string());

    let value = serde_json::to_value(error).expect("app error should serialize");

    assert_eq!(
        value,
        json!({
            "kind": "cluster",
            "message": "cluster error: context kind-dev is unavailable"
        })
    );
}
