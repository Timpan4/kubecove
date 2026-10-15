mod execution;
pub(crate) mod linting;
pub(crate) mod validation;

use crate::commands::kubeconfig::KubeconfigSource;
use crate::models::{
    AppError, KubernetesYamlLintResult, YamlApplyPreview, YamlApplyRequest, YamlApplyResult,
};

use execution::{apply_yaml_with_client, prepare_yaml_apply_with_client};
use linting::lint_kubernetes_yaml_with_kubeconform;
use validation::validate_yaml_apply;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn prepare_yaml_apply(request: YamlApplyRequest) -> Result<YamlApplyPreview, AppError> {
    let validated = validate_yaml_apply(request)?;
    let source = KubeconfigSource::new(validated.request.kubeconfig_env_var.clone())?.fail_closed();
    let client = source
        .client_for_context(&validated.request.cluster_context)
        .await?;
    prepare_yaml_apply_with_client(client, validated).await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn apply_yaml(request: YamlApplyRequest) -> Result<YamlApplyResult, AppError> {
    let validated = validate_yaml_apply(request)?;
    let source = KubeconfigSource::new(validated.request.kubeconfig_env_var.clone())?.fail_closed();
    let client = source
        .operation_client_for_context(&validated.request.cluster_context)
        .await?;
    apply_yaml_with_client(client, validated).await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn lint_kubernetes_yaml(
    request: YamlApplyRequest,
) -> Result<KubernetesYamlLintResult, AppError> {
    lint_kubernetes_yaml_with_kubeconform(request).await
}

#[cfg(test)]
#[path = "apply_client_tests.rs"]
mod apply_client_tests;
