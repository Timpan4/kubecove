use crate::commands::kubeconfig::KubeconfigSource;
use crate::models::{
    AppError, ClusterOperationPreview, ClusterOperationResult, ClusterOperationTarget,
    DeleteResourceRequest, RolloutRestartRequest, ScaleWorkloadRequest,
};
use kube::api::{Api, ApiResource, DeleteParams, DynamicObject, Patch, PatchParams};
use serde_json::json;

#[tauri::command]
pub async fn preview_scale_workload(
    request: ScaleWorkloadRequest,
) -> Result<ClusterOperationPreview, AppError> {
    let target = validate_scale(&request)?;
    let client = client_for(&target, request.kubeconfig_env_var).await?;
    let api = resource_api(client, &target)?;
    api.patch(
        &target.name,
        &PatchParams::default().dry_run(),
        &Patch::Merge(json!({ "spec": { "replicas": request.replicas } })),
    )
    .await?;
    Ok(ClusterOperationPreview {
        effect: format!(
            "Scale {} to {} replicas",
            target_label(&target),
            request.replicas
        ),
        target,
    })
}

#[tauri::command]
pub async fn scale_workload(
    request: ScaleWorkloadRequest,
) -> Result<ClusterOperationResult, AppError> {
    let target = validate_scale(&request)?;
    require_confirmation(request.confirmed)?;
    let client = operation_client_for(&target, request.kubeconfig_env_var).await?;
    resource_api(client, &target)?
        .patch(
            &target.name,
            &PatchParams::default(),
            &Patch::Merge(json!({ "spec": { "replicas": request.replicas } })),
        )
        .await?;
    Ok(ClusterOperationResult {
        effect: format!(
            "Scaled {} to {} replicas",
            target_label(&target),
            request.replicas
        ),
        target,
    })
}

#[tauri::command]
pub async fn preview_rollout_restart(
    request: RolloutRestartRequest,
) -> Result<ClusterOperationPreview, AppError> {
    let target = validate_restart(&request)?;
    let client = client_for(&target, request.kubeconfig_env_var).await?;
    restart_with_params(
        resource_api(client, &target)?,
        &target,
        PatchParams::default().dry_run(),
    )
    .await?;
    Ok(ClusterOperationPreview {
        effect: format!(
            "Restart {} by updating its pod template",
            target_label(&target)
        ),
        target,
    })
}

#[tauri::command]
pub async fn rollout_restart(
    request: RolloutRestartRequest,
) -> Result<ClusterOperationResult, AppError> {
    let target = validate_restart(&request)?;
    require_confirmation(request.confirmed)?;
    let client = operation_client_for(&target, request.kubeconfig_env_var).await?;
    restart_with_params(
        resource_api(client, &target)?,
        &target,
        PatchParams::default(),
    )
    .await?;
    Ok(ClusterOperationResult {
        effect: format!("Restarted {}", target_label(&target)),
        target,
    })
}

#[tauri::command]
pub async fn preview_delete_resource(
    request: DeleteResourceRequest,
) -> Result<ClusterOperationPreview, AppError> {
    let target = validate_delete(&request)?;
    let client = client_for(&target, request.kubeconfig_env_var).await?;
    resource_api(client, &target)?
        .delete(&target.name, &DeleteParams::default().dry_run())
        .await?;
    Ok(ClusterOperationPreview {
        effect: format!("Delete {}", target_label(&target)),
        target,
    })
}

#[tauri::command]
pub async fn delete_resource(
    request: DeleteResourceRequest,
) -> Result<ClusterOperationResult, AppError> {
    let target = validate_delete(&request)?;
    require_confirmation(request.confirmed)?;
    let client = operation_client_for(&target, request.kubeconfig_env_var).await?;
    resource_api(client, &target)?
        .delete(&target.name, &DeleteParams::default())
        .await?;
    Ok(ClusterOperationResult {
        effect: format!("Delete requested for {}", target_label(&target)),
        target,
    })
}

async fn operation_client_for(
    target: &ClusterOperationTarget,
    kubeconfig_env_var: Option<String>,
) -> Result<kube::Client, AppError> {
    KubeconfigSource::new(kubeconfig_env_var)?
        .operation_client_for_context(&target.cluster_context)
        .await
}

async fn client_for(
    target: &ClusterOperationTarget,
    kubeconfig_env_var: Option<String>,
) -> Result<kube::Client, AppError> {
    KubeconfigSource::new(kubeconfig_env_var)?
        .client_for_context(&target.cluster_context)
        .await
}

fn validate_scale(request: &ScaleWorkloadRequest) -> Result<ClusterOperationTarget, AppError> {
    validate_target(&request.target, &["Deployment", "StatefulSet"])?;
    if request.replicas < 0 {
        return Err(AppError::new(
            "replicas must be zero or greater",
            "validation",
        ));
    }
    Ok(request.target.clone())
}

fn validate_restart(request: &RolloutRestartRequest) -> Result<ClusterOperationTarget, AppError> {
    validate_target(&request.target, &["Deployment", "StatefulSet", "DaemonSet"])?;
    Ok(request.target.clone())
}

fn validate_delete(request: &DeleteResourceRequest) -> Result<ClusterOperationTarget, AppError> {
    validate_target(&request.target, &["Pod", "ConfigMap"])?;
    Ok(request.target.clone())
}

fn validate_target(target: &ClusterOperationTarget, supported: &[&str]) -> Result<(), AppError> {
    if target.cluster_context.trim().is_empty()
        || target.name.trim().is_empty()
        || target.namespace.as_deref().is_none_or(str::is_empty)
    {
        return Err(AppError::new(
            "context, namespace, and name are required",
            "validation",
        ));
    }
    if !supported.contains(&target.kind.as_str()) {
        return Err(AppError::new(
            format!("{} is not supported for this operation", target.kind),
            "unsupportedOperation",
        ));
    }
    Ok(())
}

fn require_confirmation(confirmed: bool) -> Result<(), AppError> {
    if confirmed {
        Ok(())
    } else {
        Err(AppError::new(
            "explicit confirmation is required",
            "confirmationRequired",
        ))
    }
}

fn resource_api(
    client: kube::Client,
    target: &ClusterOperationTarget,
) -> Result<Api<DynamicObject>, AppError> {
    let (group, plural) = match target.kind.as_str() {
        "Deployment" => ("apps", "deployments"),
        "StatefulSet" => ("apps", "statefulsets"),
        "DaemonSet" => ("apps", "daemonsets"),
        "Pod" => ("", "pods"),
        "ConfigMap" => ("", "configmaps"),
        _ => {
            return Err(AppError::new(
                "unsupported operation target",
                "unsupportedOperation",
            ))
        }
    };
    let resource = ApiResource {
        group: group.to_string(),
        version: "v1".to_string(),
        api_version: if group.is_empty() {
            "v1".to_string()
        } else {
            format!("{group}/v1")
        },
        kind: target.kind.clone(),
        plural: plural.to_string(),
    };
    Ok(Api::namespaced_with(
        client,
        target.namespace.as_deref().expect("validated namespace"),
        &resource,
    ))
}

async fn restart_with_params(
    api: Api<DynamicObject>,
    target: &ClusterOperationTarget,
    params: PatchParams,
) -> Result<(), AppError> {
    api.patch(
        &target.name,
        &params,
        &Patch::Merge(json!({ "spec": { "template": { "metadata": { "annotations": { "kubecove.io/restartedAt": chrono::Utc::now().to_rfc3339() } } } } })),
    ).await?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn target(kind: &str) -> ClusterOperationTarget {
        ClusterOperationTarget {
            cluster_context: "kind-dev".to_string(),
            namespace: Some("default".to_string()),
            kind: kind.to_string(),
            name: "api".to_string(),
        }
    }

    #[test]
    fn restricts_operations_to_supported_kinds() {
        let request = DeleteResourceRequest {
            target: target("Secret"),
            confirmed: false,
            kubeconfig_env_var: None,
        };
        let error = validate_delete(&request).unwrap_err();
        assert_eq!(error.kind, "unsupportedOperation");
    }

    #[test]
    fn rejects_negative_scale_and_missing_confirmation() {
        let request = ScaleWorkloadRequest {
            target: target("Deployment"),
            replicas: -1,
            confirmed: false,
            kubeconfig_env_var: None,
        };
        assert_eq!(validate_scale(&request).unwrap_err().kind, "validation");
        assert_eq!(
            require_confirmation(false).unwrap_err().kind,
            "confirmationRequired"
        );
    }
}
