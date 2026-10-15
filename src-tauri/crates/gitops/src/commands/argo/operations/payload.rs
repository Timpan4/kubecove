use crate::models::{ArgoApplicationRef, ArgoOperationRequest};
use serde_json::{json, Value};

pub(super) fn application_path(
    app: &ArgoApplicationRef,
    suffix: &str,
    refresh: Option<&str>,
) -> String {
    let mut url =
        reqwest::Url::parse("https://argo.invalid/api/v1/applications").expect("static URL");
    url.path_segments_mut().expect("static URL").push(&app.name);
    url.set_path(&format!("{}{}", url.path(), suffix));
    let mut query = url.query_pairs_mut();
    if let Some(v) = app.namespace.as_deref() {
        query.append_pair("appNamespace", v);
    }
    if let Some(v) = app.project.as_deref() {
        query.append_pair("project", v);
    }
    if let Some(v) = refresh {
        query.append_pair("refresh", v);
    }
    drop(query);
    format!(
        "{}{}",
        url.path(),
        url.query().map(|q| format!("?{q}")).unwrap_or_default()
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
        "sync" | "retry" => {
            json!({"revision":request.revision,"resources":request.resources.iter().map(identity).collect::<Vec<_>>(),"prune":request.prune,"dryRun":request.dry_run,"syncOptions":{"items":if request.force == Some(true) { vec!["Force=true", "Replace=true"] } else { vec![] }}})
        }
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
    let resource = &request.resources[0];
    let mut url = reqwest::Url::parse(&format!(
        "https://argo.invalid{}",
        application_path(&request.application, "/resource/actions", None)
    ))
    .expect("static URL");
    let mut q = url.query_pairs_mut();
    for (key, value) in [
        ("group", resource.group.as_deref()),
        ("version", resource.version.as_deref()),
        ("kind", resource.kind.as_deref()),
        ("namespace", resource.namespace.as_deref()),
        ("resourceName", resource.name.as_deref()),
    ] {
        if let Some(value) = value {
            q.append_pair(key, value);
        }
    }
    drop(q);
    format!("{}?{}", url.path(), url.query().unwrap_or_default())
}
pub(super) fn sync_payload(request: &ArgoOperationRequest) -> Value {
    json!({"revision":request.revision,"resources":request.resources.iter().map(identity).collect::<Vec<_>>(),"prune":request.prune,"dryRun":request.dry_run,"syncOptions":{"items":if request.force == Some(true) { vec!["Force=true", "Replace=true"] } else { vec![] }}})
}
