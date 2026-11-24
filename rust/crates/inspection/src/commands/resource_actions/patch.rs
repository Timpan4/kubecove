use crate::models::{
    AppError, AppErrorKind, ClusterOperationTarget, ResourceActionEdits, ResourceActionRequest,
    ResourceContainerEdit,
};
use serde_json::{Map, Value, json};

pub(super) fn validate_edits(
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

pub(super) fn build_resource_action_patch(
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

pub(super) fn complete_template_merge_patch(current: &Value, desired: &Value) -> Value {
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
