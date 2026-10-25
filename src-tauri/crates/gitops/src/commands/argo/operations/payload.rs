use super::super::transport::{application_api_path, resource_actions_path};
use crate::models::{ArgoApplicationRef, ArgoOperationRequest};
use serde_json::{json, Value};

pub(super) fn application_path(
    app: &ArgoApplicationRef,
    segments: &[&str],
    refresh: Option<&str>,
) -> String {
    application_api_path(
        &app.name,
        segments,
        &[
            ("appNamespace", app.namespace.as_deref()),
            ("project", app.project.as_deref()),
            ("refresh", refresh),
        ],
    )
}
fn identity(resource: &crate::models::ArgoManagedResource) -> Value {
    json!({"group": resource.group, "kind": resource.kind, "namespace": resource.namespace, "name": resource.name})
}
pub(super) fn connected_payload(request: &ArgoOperationRequest) -> Value {
    if matches!(request.action.as_str(), "sync" | "retry") {
        if let Some(payload) = &request.sync_payload {
            return payload.clone();
        }
    }
    match request.action.as_str() {
        "sync" | "retry" => sync_payload(request),
        "rollback" => {
            json!({"name":request.application.name,"appNamespace":request.application.namespace,"project":request.application.project,"id":request.history_id,"prune":request.prune,"dryRun":request.dry_run})
        }
        "resourceAction" => {
            let parameters = request
                .resource_action_parameters
                .as_ref()
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(name, value)| {
                    value
                        .as_str()
                        .map(|value| json!({"name":name,"value":value}))
                })
                .collect::<Vec<_>>();
            json!({"appNamespace":request.application.namespace,"project":request.application.project,"group":request.resources[0].group,"version":request.resources[0].version,"kind":request.resources[0].kind,"namespace":request.resources[0].namespace,"resourceName":request.resources[0].name,"action":request.resource_action,"resourceActionParameters":parameters})
        }
        _ => json!({}),
    }
}
pub(super) fn kubernetes_sync_payload(request: &ArgoOperationRequest) -> Value {
    let http = request
        .sync_payload
        .clone()
        .unwrap_or_else(|| sync_payload(request));
    let mut value = http.as_object().cloned().unwrap_or_default();
    let options = value
        .remove("syncOptions")
        .and_then(|value| value.get("items").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    if let Some(strategy) = value.remove("strategy") {
        value.insert("syncStrategy".into(), strategy);
    }
    value.insert("syncOptions".into(), Value::Array(options));
    Value::Object(value)
}
pub(super) fn resource_action_path(request: &ArgoOperationRequest) -> String {
    resource_actions_path(&request.application, &request.resources[0])
}
pub(super) fn sync_payload(request: &ArgoOperationRequest) -> Value {
    json!({"revision":request.revision,"resources":request.resources.iter().map(identity).collect::<Vec<_>>(),"prune":request.prune,"dryRun":request.dry_run,"syncOptions":{"items":if request.force == Some(true) { vec!["Force=true", "Replace=true"] } else { vec![] }}})
}
