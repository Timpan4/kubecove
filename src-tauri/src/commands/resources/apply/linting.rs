use crate::models::{
    AppError, KubernetesYamlLintDiagnostic, KubernetesYamlLintResult, KubernetesYamlLintSeverity,
    YamlApplyRequest,
};
use serde_json::Value;

use super::super::kubeconform::append_kubeconform_lint;
use super::validation::{parse_single_document, request_api_version};

pub(super) async fn lint_kubernetes_yaml_with_kubeconform(
    request: YamlApplyRequest,
) -> Result<KubernetesYamlLintResult, AppError> {
    let mut result = lint_kubernetes_yaml_request(request.clone());
    if !result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.source == "YAML")
    {
        append_kubeconform_lint(&request.yaml, &mut result).await;
    }
    Ok(result)
}

pub(super) fn lint_kubernetes_yaml_request(request: YamlApplyRequest) -> KubernetesYamlLintResult {
    let mut diagnostics = Vec::new();

    let manifest = match parse_single_document(&request.yaml) {
        Ok(manifest) => manifest,
        Err(err) => {
            diagnostics.push(lint_diagnostic(
                KubernetesYamlLintSeverity::Error,
                "YAML",
                err.message,
                None,
            ));
            return KubernetesYamlLintResult {
                diagnostics,
                notes: Vec::new(),
            };
        }
    };

    collect_local_lint_diagnostics(&request, &manifest, &mut diagnostics);

    KubernetesYamlLintResult {
        diagnostics,
        notes: Vec::new(),
    }
}

fn collect_local_lint_diagnostics(
    request: &YamlApplyRequest,
    manifest: &Value,
    diagnostics: &mut Vec<KubernetesYamlLintDiagnostic>,
) {
    let expected_api_version = request_api_version(request).unwrap_or_default();
    let actual_api_version = manifest.get("apiVersion").and_then(Value::as_str);
    let actual_kind = manifest.get("kind").and_then(Value::as_str);
    let metadata = manifest.get("metadata").and_then(Value::as_object);
    let actual_name = metadata
        .and_then(|metadata| metadata.get("name"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let actual_namespace = metadata
        .and_then(|metadata| metadata.get("namespace"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());

    if actual_api_version.is_none() {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Error,
            "Kubernetes",
            "apiVersion is required.",
            lint_field("apiVersion"),
        ));
    } else if actual_api_version != Some(expected_api_version.as_str()) {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Error,
            "Kubernetes",
            format!(
                "apiVersion must match selected resource: expected {}, got {}.",
                expected_api_version,
                actual_api_version.unwrap_or_default()
            ),
            lint_field("apiVersion"),
        ));
    }

    if actual_kind.is_none() {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Error,
            "Kubernetes",
            "kind is required.",
            lint_field("kind"),
        ));
    } else if actual_kind != Some(request.kind.as_str()) {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Error,
            "Kubernetes",
            format!(
                "kind must match selected resource: expected {}, got {}.",
                request.kind,
                actual_kind.unwrap_or_default()
            ),
            lint_field("kind"),
        ));
    }

    if metadata.is_none() {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Error,
            "Kubernetes",
            "metadata is required.",
            lint_field("metadata"),
        ));
        return;
    }

    if actual_name.is_none() {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Error,
            "Kubernetes",
            "metadata.name is required.",
            lint_field("metadata.name"),
        ));
    } else if actual_name != Some(request.name.as_str()) {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Error,
            "Kubernetes",
            format!(
                "metadata.name must match selected resource: expected {}, got {}.",
                request.name,
                actual_name.unwrap_or_default()
            ),
            lint_field("metadata.name"),
        ));
    }

    let namespaced = request.namespaced.unwrap_or(request.namespace.is_some());
    if namespaced {
        match request.namespace.as_deref() {
            Some(expected) if actual_namespace == Some(expected) => {}
            Some(expected) => diagnostics.push(lint_diagnostic(
                KubernetesYamlLintSeverity::Error,
                "Kubernetes",
                format!(
                    "metadata.namespace must match selected resource: expected {}, got {}.",
                    expected,
                    actual_namespace.unwrap_or("<missing>")
                ),
                lint_field("metadata.namespace"),
            )),
            None => diagnostics.push(lint_diagnostic(
                KubernetesYamlLintSeverity::Error,
                "Kubernetes",
                "metadata.namespace is required for namespaced resources.",
                lint_field("metadata.namespace"),
            )),
        }
    } else if let Some(namespace) = actual_namespace {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Error,
            "Kubernetes",
            format!(
                "cluster-scoped {} must not include metadata.namespace ({}).",
                request.kind, namespace
            ),
            lint_field("metadata.namespace"),
        ));
    }

    if request.kind == "Secret" && request.api_version.as_deref().unwrap_or("v1") == "v1" {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Warning,
            "Kubernetes",
            "v1 Secret apply is blocked because redacted values can corrupt live data.",
            lint_field("data"),
        ));
    }

    if manifest.get("status").is_some() {
        diagnostics.push(lint_diagnostic(
            KubernetesYamlLintSeverity::Warning,
            "Kubernetes",
            "Root status is server-owned and should not be applied.",
            lint_field("status"),
        ));
    }

    if let Some(metadata) = metadata {
        for field in [
            "managedFields",
            "uid",
            "resourceVersion",
            "generation",
            "creationTimestamp",
            "selfLink",
        ] {
            if metadata.get(field).is_some() {
                diagnostics.push(lint_diagnostic(
                    KubernetesYamlLintSeverity::Warning,
                    "Kubernetes",
                    format!("metadata.{field} is server-owned and should not be applied."),
                    lint_field(format!("metadata.{field}")),
                ));
            }
        }
    }
}

fn lint_diagnostic(
    severity: KubernetesYamlLintSeverity,
    source: impl Into<String>,
    message: impl Into<String>,
    field_path: Option<String>,
) -> KubernetesYamlLintDiagnostic {
    KubernetesYamlLintDiagnostic {
        severity,
        source: source.into(),
        message: message.into(),
        field_path,
    }
}

fn lint_field(path: impl Into<String>) -> Option<String> {
    Some(path.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request(yaml: &str) -> YamlApplyRequest {
        YamlApplyRequest {
            cluster_context: "kind-kind".to_string(),
            kubeconfig_env_var: None,
            kind: "Service".to_string(),
            api_version: Some("v1".to_string()),
            group: None,
            version: None,
            plural: None,
            namespaced: Some(true),
            name: "api".to_string(),
            namespace: Some("default".to_string()),
            yaml: yaml.to_string(),
            yaml_encoding: crate::models::YamlEncoding::default(),
            force_conflicts: false,
        }
    }

    #[test]
    fn lint_infers_builtin_api_version_when_request_metadata_is_missing() {
        let mut request = base_request(
            "apiVersion: v1\nkind: Service\nmetadata:\n  name: api\n  namespace: default\n",
        );
        request.api_version = None;
        request.version = None;

        let result = lint_kubernetes_yaml_request(request);

        assert!(!result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("apiVersion must match")));
    }

    #[test]
    fn lint_does_not_add_placeholder_openapi_warning() {
        let result = lint_kubernetes_yaml_request(base_request(
            "apiVersion: v1\nkind: Service\nmetadata:\n  name: api\n  namespace: default\n",
        ));

        assert!(!result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source == "OpenAPI"));
    }
}
