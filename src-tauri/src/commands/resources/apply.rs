use crate::commands::{helpers::serialize_resource_document, kubeconfig::KubeconfigSource};
use crate::models::{
    AppError, KubernetesYamlLintDiagnostic, KubernetesYamlLintResult, KubernetesYamlLintSeverity,
    YamlApplyPreview, YamlApplyRequest, YamlApplyResult, YamlApplyTarget, YamlViewMode,
};
use kube::api::{Api, ApiResource, DynamicObject, Patch, PatchParams};
use serde::Deserialize;
use serde_json::Value;

const FIELD_MANAGER: &str = "kubecove";

#[derive(Debug)]
struct ValidatedApply {
    request: YamlApplyRequest,
    manifest: Value,
    api_resource: ApiResource,
    namespaced: bool,
    target: YamlApplyTarget,
}

#[tauri::command]
pub async fn prepare_yaml_apply(request: YamlApplyRequest) -> Result<YamlApplyPreview, AppError> {
    let validated = validate_yaml_apply(request)?;
    let source = KubeconfigSource::new(validated.request.kubeconfig_env_var.clone())?;
    let client = source
        .client_for_context(&validated.request.cluster_context)
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
            &PatchParams::apply(FIELD_MANAGER).dry_run(),
            &Patch::Apply(&validated.manifest),
        )
        .await
        .map_err(AppError::from)?;
    let dry_run_yaml = serialize_resource_document(&dry_run, YamlViewMode::ApplyClean, encoding)?;

    Ok(YamlApplyPreview {
        target: validated.target,
        current_yaml,
        dry_run_yaml,
    })
}

#[tauri::command]
pub async fn apply_yaml(request: YamlApplyRequest) -> Result<YamlApplyResult, AppError> {
    let validated = validate_yaml_apply(request)?;
    let source = KubeconfigSource::new(validated.request.kubeconfig_env_var.clone())?;
    let client = source
        .client_for_context(&validated.request.cluster_context)
        .await?;
    let api = apply_api(client, &validated)?;
    let applied = api
        .patch(
            &validated.request.name,
            &PatchParams::apply(FIELD_MANAGER),
            &Patch::Apply(&validated.manifest),
        )
        .await
        .map_err(AppError::from)?;
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

#[tauri::command]
pub async fn lint_kubernetes_yaml(
    request: YamlApplyRequest,
) -> Result<KubernetesYamlLintResult, AppError> {
    Ok(lint_kubernetes_yaml_request(request))
}

fn lint_kubernetes_yaml_request(request: YamlApplyRequest) -> KubernetesYamlLintResult {
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
            return KubernetesYamlLintResult { diagnostics };
        }
    };

    collect_local_lint_diagnostics(&request, &manifest, &mut diagnostics);
    diagnostics.push(lint_diagnostic(
        KubernetesYamlLintSeverity::Warning,
        "OpenAPI",
        "OpenAPI schema lint is not available for this cluster response yet; server dry-run remains authoritative.",
        None,
    ));

    KubernetesYamlLintResult { diagnostics }
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

fn apply_api(
    client: kube::Client,
    validated: &ValidatedApply,
) -> Result<Api<DynamicObject>, AppError> {
    if validated.namespaced {
        let namespace = validated.request.namespace.as_deref().ok_or_else(|| {
            AppError::new("namespace is required for namespaced apply", "validation")
        })?;
        Ok(Api::namespaced_with(
            client,
            namespace,
            &validated.api_resource,
        ))
    } else {
        Ok(Api::all_with(client, &validated.api_resource))
    }
}

fn validate_yaml_apply(request: YamlApplyRequest) -> Result<ValidatedApply, AppError> {
    if request.kind == "Secret" && request.api_version.as_deref().unwrap_or("v1") == "v1" {
        return Err(AppError::new(
            "YAML apply is disabled for v1 Secrets because redacted values can corrupt data",
            "validation",
        ));
    }

    let manifest = parse_single_document(&request.yaml)?;
    let api_version = string_field(&manifest, "apiVersion")?;
    let kind = string_field(&manifest, "kind")?;
    let metadata = manifest
        .get("metadata")
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::new("metadata is required", "validation"))?;
    let name = metadata
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new("metadata.name is required", "validation"))?;
    let manifest_namespace = metadata
        .get("namespace")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());

    if api_version != request_api_version(&request)? {
        return Err(identity_error(
            "apiVersion",
            &request_api_version(&request)?,
            api_version,
        ));
    }
    if kind != request.kind {
        return Err(identity_error("kind", &request.kind, kind));
    }
    if name != request.name {
        return Err(identity_error("metadata.name", &request.name, name));
    }

    let (api_resource, namespaced) = api_resource_for_request(&request, &api_version)?;
    if namespaced {
        let expected_namespace = request.namespace.as_deref().ok_or_else(|| {
            AppError::new(
                "metadata.namespace is required for namespaced resources",
                "validation",
            )
        })?;
        let Some(actual_namespace) = manifest_namespace else {
            return Err(AppError::new(
                "metadata.namespace is required for namespaced resources",
                "validation",
            ));
        };
        if actual_namespace != expected_namespace {
            return Err(identity_error(
                "metadata.namespace",
                expected_namespace,
                actual_namespace,
            ));
        }
    } else if let Some(actual_namespace) = manifest_namespace {
        return Err(AppError::new(
            format!(
                "cluster-scoped {} must not include metadata.namespace ({actual_namespace})",
                request.kind
            ),
            "validation",
        ));
    }

    let target = YamlApplyTarget {
        cluster_context: request.cluster_context.clone(),
        kind: request.kind.clone(),
        api_version: Some(api_version.to_string()),
        name: request.name.clone(),
        namespace: request.namespace.clone(),
    };

    Ok(ValidatedApply {
        request,
        manifest,
        api_resource,
        namespaced,
        target,
    })
}

fn parse_single_document(yaml: &str) -> Result<Value, AppError> {
    let mut documents = Vec::new();
    for document in serde_yaml::Deserializer::from_str(yaml) {
        let yaml_value = serde_yaml::Value::deserialize(document)
            .map_err(|e| AppError::new(e.to_string(), "validation"))?;
        if matches!(yaml_value, serde_yaml::Value::Null) {
            continue;
        }
        documents.push(yaml_value);
    }

    match documents.len() {
        0 => Err(AppError::new("YAML document is empty", "validation")),
        1 => serde_json::to_value(documents.remove(0))
            .map_err(|e| AppError::new(e.to_string(), "validation")),
        _ => Err(AppError::new(
            "YAML apply accepts exactly one document",
            "validation",
        )),
    }
}

fn string_field<'a>(manifest: &'a Value, key: &str) -> Result<&'a str, AppError> {
    manifest
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new(format!("{key} is required"), "validation"))
}

fn request_api_version(request: &YamlApplyRequest) -> Result<String, AppError> {
    if let Some(api_version) = request
        .api_version
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(api_version.to_string());
    }
    match request
        .group
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        Some(group) => Ok(format!(
            "{group}/{}",
            request
                .version
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| AppError::new("version is required", "validation"))?
        )),
        None => request
            .version
            .clone()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| builtin_api_version_for_kind(&request.kind).map(str::to_string))
            .ok_or_else(|| AppError::new("apiVersion is required", "validation")),
    }
}

fn builtin_api_version_for_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "Pod"
        | "Service"
        | "ConfigMap"
        | "Secret"
        | "PersistentVolumeClaim"
        | "Node"
        | "PersistentVolume" => Some("v1"),
        "Deployment" | "StatefulSet" | "DaemonSet" => Some("apps/v1"),
        "Ingress" => Some("networking.k8s.io/v1"),
        "Job" | "CronJob" => Some("batch/v1"),
        "StorageClass" => Some("storage.k8s.io/v1"),
        _ => None,
    }
}

fn api_resource_for_request(
    request: &YamlApplyRequest,
    api_version: &str,
) -> Result<(ApiResource, bool), AppError> {
    if let Some(plural) = request
        .plural
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let namespaced = request
            .namespaced
            .ok_or_else(|| AppError::new("namespaced is required", "validation"))?;
        let (group, version) = split_api_version(api_version);
        return Ok((
            ApiResource {
                group,
                version,
                api_version: api_version.to_string(),
                kind: request.kind.clone(),
                plural: plural.to_string(),
            },
            namespaced,
        ));
    }

    builtin_api_resource(&request.kind, api_version)
}

fn builtin_api_resource(kind: &str, api_version: &str) -> Result<(ApiResource, bool), AppError> {
    let (group, version, plural, namespaced) = match (kind, api_version) {
        ("Pod", "v1") => ("", "v1", "pods", true),
        ("Service", "v1") => ("", "v1", "services", true),
        ("ConfigMap", "v1") => ("", "v1", "configmaps", true),
        ("Secret", "v1") => ("", "v1", "secrets", true),
        ("PersistentVolumeClaim", "v1") => ("", "v1", "persistentvolumeclaims", true),
        ("Node", "v1") => ("", "v1", "nodes", false),
        ("PersistentVolume", "v1") => ("", "v1", "persistentvolumes", false),
        ("Deployment", "apps/v1") => ("apps", "v1", "deployments", true),
        ("StatefulSet", "apps/v1") => ("apps", "v1", "statefulsets", true),
        ("DaemonSet", "apps/v1") => ("apps", "v1", "daemonsets", true),
        ("Ingress", "networking.k8s.io/v1") => ("networking.k8s.io", "v1", "ingresses", true),
        ("Job", "batch/v1") => ("batch", "v1", "jobs", true),
        ("CronJob", "batch/v1") => ("batch", "v1", "cronjobs", true),
        ("StorageClass", "storage.k8s.io/v1") => ("storage.k8s.io", "v1", "storageclasses", false),
        _ => {
            return Err(AppError::new(
                format!("unsupported apply target: {api_version} {kind}"),
                "validation",
            ));
        }
    };

    Ok((
        ApiResource {
            group: group.to_string(),
            version: version.to_string(),
            api_version: api_version.to_string(),
            kind: kind.to_string(),
            plural: plural.to_string(),
        },
        namespaced,
    ))
}

fn split_api_version(api_version: &str) -> (String, String) {
    api_version
        .split_once('/')
        .map(|(group, version)| (group.to_string(), version.to_string()))
        .unwrap_or_else(|| ("".to_string(), api_version.to_string()))
}

fn identity_error(field: &str, expected: &str, actual: &str) -> AppError {
    AppError::new(
        format!("{field} must match selected resource: expected {expected}, got {actual}"),
        "validation",
    )
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
            yaml_encoding: Default::default(),
        }
    }

    #[test]
    fn rejects_multi_document_yaml() {
        let err = validate_yaml_apply(base_request(
            "apiVersion: v1\nkind: Service\nmetadata:\n  name: api\n  namespace: default\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: other\n  namespace: default\n",
        ))
        .unwrap_err();

        assert_eq!(err.kind, "validation");
        assert!(err.message.contains("exactly one document"));
    }

    #[test]
    fn rejects_identity_mismatch() {
        let err = validate_yaml_apply(base_request(
            "apiVersion: v1\nkind: Service\nmetadata:\n  name: other\n  namespace: default\n",
        ))
        .unwrap_err();

        assert_eq!(err.kind, "validation");
        assert!(err.message.contains("metadata.name"));
    }

    #[test]
    fn rejects_missing_namespace_for_namespaced_resource() {
        let err = validate_yaml_apply(base_request(
            "apiVersion: v1\nkind: Service\nmetadata:\n  name: api\n",
        ))
        .unwrap_err();

        assert_eq!(err.kind, "validation");
        assert!(err.message.contains("metadata.namespace"));
    }

    #[test]
    fn rejects_v1_secret_apply() {
        let mut request = base_request(
            "apiVersion: v1\nkind: Secret\nmetadata:\n  name: api\n  namespace: default\n",
        );
        request.kind = "Secret".to_string();
        request.name = "api".to_string();

        let err = validate_yaml_apply(request).unwrap_err();

        assert_eq!(err.kind, "validation");
        assert!(err.message.contains("Secrets"));
    }

    #[test]
    fn accepts_matching_selected_resource() {
        let validated = validate_yaml_apply(base_request(
            "apiVersion: v1\nkind: Service\nmetadata:\n  name: api\n  namespace: default\nspec:\n  selector:\n    app: api\n",
        ))
        .unwrap();

        assert_eq!(validated.target.kind, "Service");
        assert!(validated.namespaced);
        assert_eq!(validated.api_resource.plural, "services");
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
}
