mod execution;
pub(crate) mod linting;
pub(crate) mod validation;

use crate::commands::{ClusterAccess, helpers::serialize_resource_document};
use crate::models::{AppError, YamlApplyPreview, YamlApplyRequest, YamlApplyResult, YamlViewMode};
use kube::api::Patch;

use execution::{
    apply_api, apply_error, apply_identity, apply_patch_params, guard_apply_manifest,
    reviewed_apply_identity, validate_apply_identity,
};
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
    let (uid, resource_version) = apply_identity(&current)?;
    let encoding = validated.request.yaml_encoding;
    let current_yaml = serialize_resource_document(&current, YamlViewMode::ApplyClean, encoding)?;
    let manifest = guard_apply_manifest(validated.manifest, &resource_version);
    let dry_run = api
        .patch(
            &validated.request.name,
            &apply_patch_params(validated.request.force_conflicts).dry_run(),
            &Patch::Apply(&manifest),
        )
        .await
        .map_err(apply_error)?;
    let dry_run_yaml = serialize_resource_document(&dry_run, YamlViewMode::ApplyClean, encoding)?;

    Ok(YamlApplyPreview {
        target: validated.target,
        current_yaml,
        dry_run_yaml,
        uid,
        resource_version,
    })
}

pub async fn apply_yaml(request: YamlApplyRequest) -> Result<YamlApplyResult, AppError> {
    let validated = validate_yaml_apply(request)?;
    let (expected_uid, expected_resource_version) = reviewed_apply_identity(&validated.request)?;
    let client = ClusterAccess::resolve(validated.request.kubeconfig_env_var.as_deref())?
        .confirmed_operation_client(&validated.request.cluster_context)
        .await?;
    let api = apply_api(client, &validated)?;
    let current = api
        .get(&validated.request.name)
        .await
        .map_err(AppError::from)?;
    let (uid, resource_version) = apply_identity(&current)?;
    validate_apply_identity(
        expected_uid,
        expected_resource_version,
        &uid,
        &resource_version,
    )?;
    let manifest = guard_apply_manifest(validated.manifest, expected_resource_version);
    api.patch(
        &validated.request.name,
        &apply_patch_params(validated.request.force_conflicts).dry_run(),
        &Patch::Apply(&manifest),
    )
    .await
    .map_err(apply_error)?;
    let applied = api
        .patch(
            &validated.request.name,
            &apply_patch_params(validated.request.force_conflicts),
            &Patch::Apply(&manifest),
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

#[cfg(test)]
#[path = "apply_client_tests.rs"]
mod apply_client_tests;
