mod execution;
pub(crate) mod linting;
pub(crate) mod validation;

use crate::commands::{helpers::serialize_resource_document, ClusterAccess};
use crate::models::{
    AppError, KubernetesYamlLintResult, YamlApplyPreview, YamlApplyRequest, YamlApplyResult,
    YamlViewMode,
};
use kube::api::Patch;

use execution::{apply_api, apply_error, apply_patch_params};
use linting::lint_kubernetes_yaml_with_kubeconform;
use validation::validate_yaml_apply;

pub async fn prepare_yaml_apply(request: YamlApplyRequest) -> Result<YamlApplyPreview, AppError> {
    let validated = validate_yaml_apply(request)?;
    let client = ClusterAccess::resolve(validated.request.kubeconfig_env_var.as_deref())?
        .preview_client(&validated.request.cluster_context)
        .await?;
    let api = apply_api(client, &validated)?;
    let current = api
        .get(&validated.request.name)
        .await
        .map_err(AppError::from)?;
    let encoding = validated.request.yaml_encoding;
    let current_yaml = serialize_resource_document(&current, YamlViewMode::ApplyClean, encoding)?;
    let dry_run = api
        .patch(
            &validated.request.name,
            &apply_patch_params(validated.request.force_conflicts).dry_run(),
            &Patch::Apply(&validated.manifest),
        )
        .await
        .map_err(apply_error)?;
    let dry_run_yaml = serialize_resource_document(&dry_run, YamlViewMode::ApplyClean, encoding)?;

    Ok(YamlApplyPreview {
        target: validated.target,
        current_yaml,
        dry_run_yaml,
    })
}

pub async fn apply_yaml(request: YamlApplyRequest) -> Result<YamlApplyResult, AppError> {
    let validated = validate_yaml_apply(request)?;
    let client = ClusterAccess::resolve(validated.request.kubeconfig_env_var.as_deref())?
        .confirmed_operation_client(&validated.request.cluster_context)
        .await?;
    let applied = apply_api(client, &validated)?
        .patch(
            &validated.request.name,
            &apply_patch_params(validated.request.force_conflicts),
            &Patch::Apply(&validated.manifest),
        )
        .await
        .map_err(apply_error)?;
    let applied_yaml = serialize_resource_document(
        &applied,
        YamlViewMode::ApplyClean,
        validated.request.yaml_encoding,
    )?;

    Ok(YamlApplyResult {
        target: validated.target,
        applied_yaml,
    })
}

pub async fn lint_kubernetes_yaml(
    request: YamlApplyRequest,
) -> Result<KubernetesYamlLintResult, AppError> {
    lint_kubernetes_yaml_with_kubeconform(request).await
}

#[cfg(test)]
#[path = "apply_client_tests.rs"]
mod apply_client_tests;
