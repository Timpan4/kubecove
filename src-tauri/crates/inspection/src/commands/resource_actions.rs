use crate::commands::builtin_kinds::{builtin_kind, BuiltinKind};
use crate::commands::{operations::preview_patch_params, ClusterAccess};
use crate::models::{
    AppError, AppErrorKind, ClusterOperationResult, ClusterOperationTarget, ResourceActionEdits,
    ResourceActionKind, ResourceActionPreview, ResourceActionRequest, ResourceActionState,
    ResourceActionStateRequest, ResourceContainerEdit,
};
use kube::api::{Api, ApiResource, DeleteParams, DynamicObject, Patch, PatchParams};
use serde_json::{json, Map, Value};

const ACTION_KINDS: &[&str] = &["Deployment", "StatefulSet", "DaemonSet", "Pod", "ConfigMap"];

pub async fn get_resource_action_state(
    request: ResourceActionStateRequest,
) -> Result<ResourceActionState, AppError> {
    validate_resource_action_target(&request.target)?;
    let client = ClusterAccess::resolve(request.kubeconfig_env_var.as_deref())?
        .preview_client(&request.target.cluster_context)
        .await?;
    let api = resource_api(client.clone(), &request.target)?;
    let current = api.get(&request.target.name).await?;
    let value = action_value(&current)?;
    ensure_action_resource_is_not_secret(&value)?;
    let (uid, resource_version) = action_identity(&current)?;

    let mut permissions = Vec::new();
    for (action, verb, resource) in permissions_for_kind(&request.target.kind) {
        let reviewed = access_review(client.clone(), &request.target, verb, resource).await;
        permissions.push(match reviewed {
            Ok(allowed) => permission_from_review(action, verb, resource, Some(allowed), None),
            Err(error) => {
                permission_from_review(action, verb, resource, None, Some(&error.message))
            }
        });
    }

    let (revisions, revision_error) = if request.target.kind == "Deployment" {
        match owned_revisions(client.clone(), &request.target, &uid).await {
            Ok(revisions) => (revisions, None),
            Err(error) => (Vec::new(), Some(error)),
        }
    } else {
        (Vec::new(), None)
    };
    if let Some(error) = revision_error {
        set_rollback_unavailable(
            &mut permissions,
            &format!("Revision lookup failed: {}", error.message),
        );
    }
    if request.target.kind == "Deployment" {
        match access_review(client, &request.target, "list", "replicasets").await {
            Ok(true) => set_rollback_permission_text(
                &mut permissions,
                "patch deployments and list replicasets",
            ),
            Ok(false) => {
                set_rollback_unavailable(&mut permissions, "Kubernetes denied list replicasets");
            }
            Err(error) => set_rollback_unavailable(
                &mut permissions,
                &format!("Revision permission check failed: {}", error.message),
            ),
        }
    }

    Ok(state_from_value(
        request.target,
        uid,
        resource_version,
        value,
        permissions,
        revisions,
    ))
}

pub async fn preview_resource_action(
    request: ResourceActionRequest,
) -> Result<ResourceActionPreview, AppError> {
    validate_resource_action_request(&request)?;
    let client = ClusterAccess::resolve(request.kubeconfig_env_var.as_deref())?
        .preview_client(&request.target.cluster_context)
        .await?;
    let api = resource_api(client.clone(), &request.target)?;
    let current = api.get(&request.target.name).await?;
    let current_value = action_value(&current)?;
    ensure_action_resource_is_not_secret(&current_value)?;
    let (uid, resource_version) = action_identity(&current)?;
    if request.expected_uid.is_some() || request.expected_resource_version.is_some() {
        validate_review_guard(&request, &uid, &resource_version)?;
    }
    let before_yaml = action_yaml(&current_value)?;
    let (effect, after_yaml, revision_identity) = preview_effect(
        &request,
        api,
        client,
        &current_value,
        &uid,
        &resource_version,
    )
    .await?;
    Ok(ResourceActionPreview {
        target: request.target,
        effect,
        before_yaml,
        after_yaml,
        uid,
        resource_version,
        revision_uid: revision_identity.as_ref().map(|(uid, _)| uid.clone()),
        revision_resource_version: revision_identity.map(|(_, version)| version),
    })
}

pub async fn execute_resource_action(
    request: ResourceActionRequest,
) -> Result<ClusterOperationResult, AppError> {
    validate_resource_action_request(&request)?;
    require_action_confirmation(&request)?;
    let client = ClusterAccess::resolve(request.kubeconfig_env_var.as_deref())?
        .confirmed_operation_client(&request.target.cluster_context)
        .await?;
    if request.action == ResourceActionKind::Rollback
        && request
            .expected_revision_uid
            .as_deref()
            .zip(request.expected_revision_resource_version.as_deref())
            .is_none()
    {
        return Err(AppError::new(
            "reviewed revision UID and resourceVersion are required",
            AppErrorKind::Validation,
        ));
    }
    let (expected_uid, expected_resource_version) = request
        .expected_uid
        .as_deref()
        .zip(request.expected_resource_version.as_deref())
        .ok_or_else(|| {
            AppError::new(
                "reviewed UID and resourceVersion are required",
                AppErrorKind::Validation,
            )
        })?;
    let api = resource_api(client.clone(), &request.target)?;
    let current = api.get(&request.target.name).await?;
    let current_value = action_value(&current)?;
    ensure_action_resource_is_not_secret(&current_value)?;
    let (uid, resource_version) = action_identity(&current)?;
    validate_review_guard(&request, &uid, &resource_version)?;

    if request.action == ResourceActionKind::Delete {
        api.delete(
            &request.target.name,
            &preview_exact_delete_params(expected_uid, expected_resource_version),
        )
        .await?;
        api.delete(
            &request.target.name,
            &exact_delete_params(expected_uid, expected_resource_version),
        )
        .await?;
    } else {
        let (patch, _) = action_patch(
            &request,
            &current_value,
            &uid,
            &resource_version,
            client.clone(),
        )
        .await?;
        api.patch(&request.target.name, &preview_patch_params(), &patch)
            .await?;
        api.patch(&request.target.name, &PatchParams::default(), &patch)
            .await?;
    }
    Ok(ClusterOperationResult {
        effect: format!(
            "{} {}",
            action_verb(request.action),
            target_label(&request.target)
        ),
        target: request.target,
    })
}

async fn preview_effect(
    request: &ResourceActionRequest,
    api: Api<DynamicObject>,
    client: kube::Client,
    current: &Value,
    uid: &str,
    resource_version: &str,
) -> Result<(String, String, Option<(String, String)>), AppError> {
    if request.action == ResourceActionKind::Delete {
        let response = api
            .delete(
                &request.target.name,
                &preview_exact_delete_params(uid, resource_version),
            )
            .await?;
        let value = if response.is_left() {
            let object = response.left().ok_or_else(|| {
                AppError::new(
                    "delete preview returned no resource",
                    AppErrorKind::Serialization,
                )
            })?;
            action_value(&object)?
        } else {
            let status = response.right().ok_or_else(|| {
                AppError::new(
                    "delete preview returned no status",
                    AppErrorKind::Serialization,
                )
            })?;
            serde_json::to_value(status)
                .map_err(|error| AppError::new(error.to_string(), AppErrorKind::Serialization))?
        };
        let yaml = action_yaml(&value)?;
        return Ok((
            format!("Delete {}", target_label(&request.target)),
            yaml,
            None,
        ));
    }
    let (patch, revision_identity) =
        action_patch(request, current, uid, resource_version, client).await?;
    let result = api
        .patch(&request.target.name, &preview_patch_params(), &patch)
        .await?;
    let after = action_value(&result)?;
    let verb = action_verb(request.action);
    Ok((
        format!("{verb} {}", target_label(&request.target)),
        action_yaml(&after)?,
        revision_identity,
    ))
}

async fn action_patch(
    request: &ResourceActionRequest,
    current: &Value,
    uid: &str,
    resource_version: &str,
    client: kube::Client,
) -> Result<(Patch<serde_json::Value>, Option<(String, String)>), AppError> {
    let mut revision_identity = None;
    let mut patch = match request.action {
        ResourceActionKind::Edit => build_resource_action_patch(
            request,
            request
                .edits
                .as_ref()
                .ok_or_else(|| AppError::new("edits are required", AppErrorKind::Validation))?,
            current,
        )?,
        ResourceActionKind::Restart => json!({ "spec": { "template": { "metadata": {
            "annotations": { "kubecove.io/restartedAt": chrono::Utc::now().to_rfc3339() }
        } } } }),
        ResourceActionKind::Pause | ResourceActionKind::Resume => {
            json!({ "spec": { "paused": request.action == ResourceActionKind::Pause } })
        }
        ResourceActionKind::Rollback => {
            let revision = request
                .revision
                .ok_or_else(|| AppError::new("revision is required", AppErrorKind::Validation))?;
            let replica_set =
                owned_revision_by_number(client, &request.target, uid, revision).await?;
            let source_identity = revision_source_identity(&replica_set)?;
            if request.expected_revision_uid.is_some()
                || request.expected_revision_resource_version.is_some()
            {
                validate_revision_review_guard(request, &source_identity.0, &source_identity.1)?;
            }
            revision_identity = Some(source_identity);
            let template = owned_revision_template(&replica_set, &request.target.name, uid)?;
            let current_template = current.pointer("/spec/template").ok_or_else(|| {
                AppError::new("Deployment has no pod template", AppErrorKind::Validation)
            })?;
            json!({ "spec": { "template": complete_template_merge_patch(current_template, &template) } })
        }
        ResourceActionKind::Delete => unreachable!("delete uses DeleteParams"),
    };
    if let Some(root) = patch.as_object_mut() {
        let metadata = root.entry("metadata").or_insert_with(|| json!({}));
        metadata["uid"] = json!(uid);
        metadata["resourceVersion"] = json!(resource_version);
    }
    Ok((Patch::Merge(patch), revision_identity))
}

fn validate_resource_action_request(request: &ResourceActionRequest) -> Result<(), AppError> {
    let target = &request.target;
    validate_resource_action_target(target)?;
    let allowed_action = match request.action {
        ResourceActionKind::Edit => true,
        ResourceActionKind::Restart => matches!(
            target.kind.as_str(),
            "Deployment" | "StatefulSet" | "DaemonSet"
        ),
        ResourceActionKind::Pause | ResourceActionKind::Resume => target.kind == "Deployment",
        ResourceActionKind::Rollback => target.kind == "Deployment",
        ResourceActionKind::Delete => true,
    };
    if !allowed_action {
        return Err(AppError::new(
            format!(
                "{} is not supported for {}",
                action_name(request.action),
                target.kind
            ),
            AppErrorKind::UnsupportedOperation,
        ));
    }
    if request.action == ResourceActionKind::Rollback
        && request.revision.is_none_or(|revision| revision < 1)
    {
        return Err(AppError::new(
            "a positive revision is required",
            AppErrorKind::Validation,
        ));
    }
    if let Some(edits) = request.edits.as_ref() {
        validate_edits(target, edits)?;
    }
    Ok(())
}

fn validate_resource_action_target(target: &ClusterOperationTarget) -> Result<(), AppError> {
    crate::commands::helpers::validate_namespace(target.namespace.as_deref())?;
    crate::commands::helpers::validate_path_segment(&target.name, "name")?;
    if target.cluster_context.trim().is_empty()
        || target.name.trim().is_empty()
        || target.namespace.as_deref().is_none_or(str::is_empty)
    {
        return Err(AppError::new(
            "context, namespace, and name are required",
            AppErrorKind::Validation,
        ));
    }
    if !ACTION_KINDS.contains(&target.kind.as_str()) {
        return Err(AppError::new(
            format!("{} is not supported for resource actions", target.kind),
            AppErrorKind::UnsupportedOperation,
        ));
    }
    let kind = builtin_kind(&target.kind).ok_or_else(|| {
        AppError::new(
            "unsupported action target",
            AppErrorKind::UnsupportedOperation,
        )
    })?;
    if target
        .api_version
        .as_deref()
        .is_some_and(|version| version != kind.api_version())
    {
        return Err(AppError::new(
            format!(
                "{} is not the built-in {} resource",
                target.api_version.as_deref().unwrap_or_default(),
                target.kind
            ),
            AppErrorKind::UnsupportedOperation,
        ));
    }
    Ok(())
}

fn validate_edits(
    target: &ClusterOperationTarget,
    edits: &ResourceActionEdits,
) -> Result<(), AppError> {
    if edits.replicas.is_some_and(|replicas| replicas < 0) {
        return Err(AppError::new(
            "replicas must be zero or greater",
            AppErrorKind::Validation,
        ));
    }
    if edits.replicas.is_some() && !matches!(target.kind.as_str(), "Deployment" | "StatefulSet") {
        return Err(AppError::new(
            "replicas are supported only for Deployments and StatefulSets",
            AppErrorKind::UnsupportedOperation,
        ));
    }
    if (!edits.containers.is_empty())
        && !matches!(
            target.kind.as_str(),
            "Deployment" | "StatefulSet" | "DaemonSet"
        )
    {
        return Err(AppError::new(
            "container edits are supported only for workloads",
            AppErrorKind::UnsupportedOperation,
        ));
    }
    for container in &edits.containers {
        if container.name.trim().is_empty() {
            return Err(AppError::new(
                "container name is required",
                AppErrorKind::Validation,
            ));
        }
        for env in &container.env {
            if env.name.trim().is_empty() || (env.remove != Some(true) && env.value.is_none()) {
                return Err(AppError::new(
                    "environment edits require a name and either a value or remove=true",
                    AppErrorKind::Validation,
                ));
            }
        }
    }
    if edits
        .containers
        .iter()
        .any(|c| c.image.as_ref().is_some_and(|v| v.trim().is_empty()))
    {
        return Err(AppError::new(
            "container image cannot be empty",
            AppErrorKind::Validation,
        ));
    }
    Ok(())
}

fn validate_review_guard(
    request: &ResourceActionRequest,
    uid: &str,
    resource_version: &str,
) -> Result<(), AppError> {
    if request.expected_uid.as_deref() != Some(uid)
        || request.expected_resource_version.as_deref() != Some(resource_version)
    {
        return Err(AppError::new(
            "resource changed since the action was reviewed",
            AppErrorKind::Validation,
        ));
    }
    Ok(())
}

fn require_action_confirmation(request: &ResourceActionRequest) -> Result<(), AppError> {
    if request.confirmed {
        Ok(())
    } else {
        Err(AppError::new(
            "explicit confirmation is required",
            AppErrorKind::ConfirmationRequired,
        ))
    }
}

fn build_resource_action_patch(
    request: &ResourceActionRequest,
    edits: &ResourceActionEdits,
    current: &Value,
) -> Result<Value, AppError> {
    validate_edits(&request.target, edits)?;
    let mut patch = Map::new();
    let mut metadata = Map::new();
    for (field, delta) in [
        ("labels", &edits.labels),
        ("annotations", &edits.annotations),
    ] {
        if !delta.is_empty() {
            let mut values = Map::new();
            for (key, value) in delta {
                if key.is_empty() {
                    return Err(AppError::new(
                        "metadata keys cannot be empty",
                        AppErrorKind::Validation,
                    ));
                }
                values.insert(
                    key.clone(),
                    value.as_ref().map_or(Value::Null, |value| json!(value)),
                );
            }
            metadata.insert(field.to_string(), Value::Object(values));
        }
    }
    if let Some(resource_version) = current.pointer("/metadata/resourceVersion") {
        metadata.insert("resourceVersion".into(), resource_version.clone());
    }
    if let Some(uid) = current.pointer("/metadata/uid") {
        metadata.insert("uid".into(), uid.clone());
    }
    if !metadata.is_empty() {
        patch.insert("metadata".into(), Value::Object(metadata));
    }

    if edits.replicas.is_some() {
        patch.insert("spec".into(), json!({"replicas": edits.replicas}));
    }
    if !edits.containers.is_empty() {
        let container_path = if request.target.kind == "Pod" {
            "/spec/containers"
        } else {
            "/spec/template/spec/containers"
        };
        let mut containers = current
            .pointer(container_path)
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "resource has no regular containers",
                    AppErrorKind::Validation,
                )
            })?;
        for edit in &edits.containers {
            apply_container_edit(&mut containers, edit)?;
        }
        let object = if request.target.kind == "Pod" {
            patch.entry("spec").or_insert_with(|| json!({}))
        } else {
            patch
                .entry("spec")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .expect("object patch")
                .entry("template")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .expect("object template")
                .entry("spec")
                .or_insert_with(|| json!({}))
        };
        object
            .as_object_mut()
            .ok_or_else(|| AppError::new("invalid container patch", AppErrorKind::Internal))?
            .insert("containers".into(), Value::Array(containers));
    }
    if patch.is_empty() {
        return Err(AppError::new(
            "at least one edit is required",
            AppErrorKind::Validation,
        ));
    }
    Ok(Value::Object(patch))
}

fn apply_container_edit(
    containers: &mut [Value],
    edit: &ResourceContainerEdit,
) -> Result<(), AppError> {
    let container = containers
        .iter_mut()
        .find(|container| container.get("name").and_then(Value::as_str) == Some(edit.name.as_str()))
        .ok_or_else(|| {
            AppError::new(
                format!("container {} was not found", edit.name),
                AppErrorKind::Validation,
            )
        })?;
    if let Some(image) = &edit.image {
        container["image"] = json!(image);
    }
    if !edit.env.is_empty() {
        let env = container
            .as_object_mut()
            .ok_or_else(|| AppError::new("container entry is invalid", AppErrorKind::Validation))?
            .entry("env")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| {
                AppError::new(
                    "container environment field is invalid",
                    AppErrorKind::Validation,
                )
            })?;
        for change in &edit.env {
            let index = env.iter().position(|entry| {
                entry.get("name").and_then(Value::as_str) == Some(change.name.as_str())
            });
            if change.remove == Some(true) {
                if let Some(index) = index {
                    env.remove(index);
                }
                continue;
            }
            let mut entry =
                index.map_or_else(|| json!({"name": change.name}), |index| env.remove(index));
            entry
                .as_object_mut()
                .expect("environment entries are objects")
                .remove("valueFrom");
            entry["value"] = json!(change.value.as_deref().unwrap_or_default());
            let insertion = index.unwrap_or(env.len()).min(env.len());
            env.insert(insertion, entry);
        }
    }
    for (field, delta) in [("requests", &edit.requests), ("limits", &edit.limits)] {
        if !delta.is_empty() {
            let mut resources = container
                .get("resources")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let mut values = resources
                .get(field)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            for (name, value) in delta {
                match value {
                    Some(value) => {
                        values.insert(name.clone(), json!(value));
                    }
                    None => {
                        values.remove(name);
                    }
                }
            }
            resources.insert(field.into(), Value::Object(values));
            container["resources"] = Value::Object(resources);
        }
    }
    Ok(())
}

pub(super) fn owned_revision_template(
    revision: &Value,
    deployment_name: &str,
    deployment_uid: &str,
) -> Result<Value, AppError> {
    let owned = revision
        .pointer("/metadata/ownerReferences")
        .and_then(Value::as_array)
        .is_some_and(|owners| {
            owners.iter().any(|owner| {
                owner.get("kind").and_then(Value::as_str) == Some("Deployment")
                    && owner.get("name").and_then(Value::as_str) == Some(deployment_name)
                    && owner.get("uid").and_then(Value::as_str) == Some(deployment_uid)
                    && owner.get("controller").and_then(Value::as_bool) == Some(true)
            })
        });
    if !owned {
        return Err(AppError::new(
            "ReplicaSet is not owned by the selected Deployment",
            AppErrorKind::Validation,
        ));
    }
    let mut template = revision
        .pointer("/spec/template")
        .cloned()
        .ok_or_else(|| AppError::new("ReplicaSet has no pod template", AppErrorKind::Validation))?;
    if let Some(labels) = template
        .pointer_mut("/metadata/labels")
        .and_then(Value::as_object_mut)
    {
        labels.remove("pod-template-hash");
    }
    Ok(template)
}

fn complete_template_merge_patch(current: &Value, desired: &Value) -> Value {
    match (current.as_object(), desired.as_object()) {
        (Some(current), Some(desired)) => {
            let mut patch = Map::new();
            for (key, old) in current {
                if let Some(new) = desired.get(key) {
                    let delta = complete_template_merge_patch(old, new);
                    if delta != json!({}) || old != new {
                        patch.insert(key.clone(), delta);
                    }
                } else {
                    patch.insert(key.clone(), Value::Null);
                }
            }
            for (key, new) in desired {
                if !current.contains_key(key) {
                    patch.insert(key.clone(), new.clone());
                }
            }
            Value::Object(patch)
        }
        _ => desired.clone(),
    }
}

mod state;
use state::{
    access_review, action_identity, action_value, action_yaml,
    ensure_action_resource_is_not_secret, owned_revision_by_number, owned_revisions,
    permission_from_review, permissions_for_kind, revision_source_identity,
    set_rollback_permission_text, set_rollback_unavailable, state_from_value,
    validate_revision_review_guard,
};

fn resource_api(
    client: kube::Client,
    target: &ClusterOperationTarget,
) -> Result<Api<DynamicObject>, AppError> {
    Ok(Api::namespaced_with(
        client,
        target
            .namespace
            .as_deref()
            .ok_or_else(|| AppError::new("namespace is required", AppErrorKind::Validation))?,
        &kind_api_resource(&target.kind)?,
    ))
}

pub(super) fn kind_api_resource(kind: &str) -> Result<ApiResource, AppError> {
    builtin_kind(kind)
        .map(BuiltinKind::api_resource)
        .ok_or_else(|| {
            AppError::new(
                "unsupported action target",
                AppErrorKind::UnsupportedOperation,
            )
        })
}

fn exact_delete_params(uid: &str, resource_version: &str) -> DeleteParams {
    DeleteParams {
        dry_run: false,
        grace_period_seconds: None,
        propagation_policy: None,
        preconditions: Some(kube::api::Preconditions {
            uid: Some(uid.to_string()),
            resource_version: Some(resource_version.to_string()),
        }),
    }
}

fn preview_exact_delete_params(uid: &str, resource_version: &str) -> DeleteParams {
    DeleteParams {
        dry_run: true,
        ..exact_delete_params(uid, resource_version)
    }
}

fn target_label(target: &ClusterOperationTarget) -> String {
    format!(
        "{}/{}/{} in context {}",
        target.kind,
        target.namespace.as_deref().unwrap_or("-"),
        target.name,
        target.cluster_context
    )
}
fn action_name(action: ResourceActionKind) -> &'static str {
    match action {
        ResourceActionKind::Edit => "edit",
        ResourceActionKind::Restart => "restart",
        ResourceActionKind::Pause => "pause",
        ResourceActionKind::Resume => "resume",
        ResourceActionKind::Rollback => "rollback",
        ResourceActionKind::Delete => "delete",
    }
}
fn action_verb(action: ResourceActionKind) -> &'static str {
    match action {
        ResourceActionKind::Edit => "Update",
        ResourceActionKind::Restart => "Restart",
        ResourceActionKind::Pause => "Pause",
        ResourceActionKind::Resume => "Resume",
        ResourceActionKind::Rollback => "Roll back",
        ResourceActionKind::Delete => "Delete",
    }
}

#[cfg(test)]
#[path = "resource_actions_tests.rs"]
mod tests;
