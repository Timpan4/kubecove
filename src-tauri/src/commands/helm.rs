use crate::commands::{
    helpers::{k8s_creation_timestamp_to_rfc3339, list_params, resource_age},
    kubeconfig::KubeconfigSource,
};
use crate::models::{
    AppError, HelmManifestResourceSummary, HelmManifestSummary, HelmReleaseDetails,
    HelmReleaseSummary, HelmValuesSummary,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use k8s_openapi::api::core::v1::{ConfigMap, Namespace, Secret};
use kube::{
    api::{Api, ListParams, ObjectMeta},
    Client,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read};

const HELM_OWNER_SELECTOR: &str = "owner=helm";
const HELM_STORAGE_SECRET: &str = "Secret";
const HELM_STORAGE_CONFIGMAP: &str = "ConfigMap";

#[derive(Debug, Serialize, Deserialize)]
struct DecodedHelmRelease {
    name: Option<String>,
    namespace: Option<String>,
    version: Option<i32>,
    info: Option<DecodedHelmInfo>,
    chart: Option<DecodedHelmChart>,
    config: Option<serde_json::Value>,
    manifest: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DecodedHelmInfo {
    status: Option<String>,
    last_deployed: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DecodedHelmChart {
    metadata: Option<DecodedHelmChartMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecodedHelmChartMetadata {
    name: Option<String>,
    version: Option<String>,
    app_version: Option<String>,
}

#[derive(Debug, Clone)]
struct HelmStorageRecord {
    summary: HelmReleaseSummary,
    release: Option<serde_json::Value>,
    values_summary: HelmValuesSummary,
    manifest_summary: HelmManifestSummary,
}

#[tauri::command]
pub async fn list_helm_releases(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<Vec<HelmReleaseSummary>, AppError> {
    let (client, default_namespace) =
        client_for_context(&cluster_context, kubeconfig_env_var).await?;
    let fallback_namespaces = helm_fallback_namespaces(client.clone(), &default_namespace).await;
    let mut records = Vec::new();
    let mut errors = Vec::new();

    match list_secret_releases(client.clone(), &cluster_context, &fallback_namespaces).await {
        Ok(mut releases) => records.append(&mut releases),
        Err(err) => errors.push(err.message),
    }

    match list_configmap_releases(client, &cluster_context, &fallback_namespaces).await {
        Ok(mut releases) => records.append(&mut releases),
        Err(err) => errors.push(err.message),
    }

    if records.is_empty() && !errors.is_empty() {
        return Err(AppError::kube(errors.join("; ")));
    }

    Ok(latest_releases(records))
}

#[tauri::command]
pub async fn get_helm_release_details(
    cluster_context: String,
    namespace: String,
    storage_kind: String,
    storage_name: String,
    kubeconfig_env_var: Option<String>,
) -> Result<HelmReleaseDetails, AppError> {
    let (client, _) = client_for_context(&cluster_context, kubeconfig_env_var).await?;

    match storage_kind.as_str() {
        HELM_STORAGE_SECRET => {
            let api: Api<Secret> = Api::namespaced(client, &namespace);
            let mut secret = api.get(&storage_name).await.map_err(AppError::from)?;
            if !is_helm_owned(secret.metadata.labels.as_ref()) {
                return Err(AppError::new(
                    "storage object is not a Helm release",
                    "validation",
                ));
            }
            let record = secret_record(&cluster_context, &mut secret)?;
            let metadata = serde_json::to_value(&secret.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            redact_secret_release(&mut secret);
            let yaml = serde_yaml::to_string(&secret)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            Ok(HelmReleaseDetails {
                summary: record.summary,
                yaml,
                metadata,
                values_summary: record.values_summary,
                manifest_summary: record.manifest_summary,
                release: record.release,
            })
        }
        HELM_STORAGE_CONFIGMAP => {
            let api: Api<ConfigMap> = Api::namespaced(client, &namespace);
            let mut configmap = api.get(&storage_name).await.map_err(AppError::from)?;
            if !is_helm_owned(configmap.metadata.labels.as_ref()) {
                return Err(AppError::new(
                    "storage object is not a Helm release",
                    "validation",
                ));
            }
            let record = configmap_record(&cluster_context, &mut configmap)?;
            let metadata = serde_json::to_value(&configmap.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            redact_configmap_release(&mut configmap);
            let yaml = serde_yaml::to_string(&configmap)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            Ok(HelmReleaseDetails {
                summary: record.summary,
                yaml,
                metadata,
                values_summary: record.values_summary,
                manifest_summary: record.manifest_summary,
                release: record.release,
            })
        }
        _ => Err(AppError::new("unsupported Helm storage kind", "validation")),
    }
}

async fn client_for_context(
    cluster_context: &str,
    kubeconfig_env_var: Option<String>,
) -> Result<(Client, String), AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    source.client_and_default_namespace(cluster_context).await
}

async fn helm_fallback_namespaces(client: Client, default_namespace: &str) -> Vec<String> {
    let api: Api<Namespace> = Api::all(client);
    match api.list(&list_params()).await {
        Ok(items) => {
            let namespaces: Vec<String> = items
                .items
                .into_iter()
                .filter_map(|namespace| namespace.metadata.name)
                .collect();
            if namespaces.is_empty() && !default_namespace.is_empty() {
                vec![default_namespace.to_string()]
            } else {
                namespaces
            }
        }
        Err(_) if !default_namespace.is_empty() => vec![default_namespace.to_string()],
        Err(_) => Vec::new(),
    }
}

async fn list_secret_releases(
    client: Client,
    cluster_context: &str,
    fallback_namespaces: &[String],
) -> Result<Vec<HelmStorageRecord>, AppError> {
    let api: Api<Secret> = Api::all(client.clone());
    let params = helm_list_params();
    let items = match api.list(&params).await {
        Ok(items) => items,
        Err(all_error) => {
            return list_secret_releases_by_namespace(client, cluster_context, fallback_namespaces)
                .await
                .map_err(|namespace_error| {
                    AppError::kube(format!(
                        "{}; {}",
                        AppError::from(all_error).message,
                        namespace_error.message
                    ))
                });
        }
    };
    items
        .items
        .into_iter()
        .map(|mut secret| secret_record(cluster_context, &mut secret))
        .collect()
}

async fn list_configmap_releases(
    client: Client,
    cluster_context: &str,
    fallback_namespaces: &[String],
) -> Result<Vec<HelmStorageRecord>, AppError> {
    let api: Api<ConfigMap> = Api::all(client.clone());
    let params = helm_list_params();
    let items = match api.list(&params).await {
        Ok(items) => items,
        Err(all_error) => {
            return list_configmap_releases_by_namespace(
                client,
                cluster_context,
                fallback_namespaces,
            )
            .await
            .map_err(|namespace_error| {
                AppError::kube(format!(
                    "{}; {}",
                    AppError::from(all_error).message,
                    namespace_error.message
                ))
            });
        }
    };
    items
        .items
        .into_iter()
        .map(|mut configmap| configmap_record(cluster_context, &mut configmap))
        .collect()
}

async fn list_secret_releases_by_namespace(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
) -> Result<Vec<HelmStorageRecord>, AppError> {
    let params = helm_list_params();
    let mut records = Vec::new();
    let mut succeeded = false;
    let mut errors = Vec::new();

    for namespace in namespaces {
        let api: Api<Secret> = Api::namespaced(client.clone(), namespace);
        match api.list(&params).await {
            Ok(items) => {
                succeeded = true;
                for mut secret in items.items {
                    records.push(secret_record(cluster_context, &mut secret)?);
                }
            }
            Err(err) => errors.push(format!("{}: {}", namespace, AppError::from(err).message)),
        }
    }

    if !succeeded && !errors.is_empty() {
        Err(AppError::kube(errors.join("; ")))
    } else {
        Ok(records)
    }
}

async fn list_configmap_releases_by_namespace(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
) -> Result<Vec<HelmStorageRecord>, AppError> {
    let params = helm_list_params();
    let mut records = Vec::new();
    let mut succeeded = false;
    let mut errors = Vec::new();

    for namespace in namespaces {
        let api: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
        match api.list(&params).await {
            Ok(items) => {
                succeeded = true;
                for mut configmap in items.items {
                    records.push(configmap_record(cluster_context, &mut configmap)?);
                }
            }
            Err(err) => errors.push(format!("{}: {}", namespace, AppError::from(err).message)),
        }
    }

    if !succeeded && !errors.is_empty() {
        Err(AppError::kube(errors.join("; ")))
    } else {
        Ok(records)
    }
}

fn helm_list_params() -> ListParams {
    list_params().labels(HELM_OWNER_SELECTOR)
}

fn secret_record(
    cluster_context: &str,
    secret: &mut Secret,
) -> Result<HelmStorageRecord, AppError> {
    let release_data = secret
        .data
        .as_ref()
        .and_then(|data| data.get("release"))
        .map(|data| data.0.as_slice());
    let decoded = release_data.and_then(decode_helm_release);
    let release = decoded.as_ref().and_then(safe_release_metadata);
    let values_summary =
        values_summary(decoded.as_ref().and_then(|release| release.config.as_ref()));
    let manifest_summary = manifest_summary(
        decoded
            .as_ref()
            .and_then(|release| release.manifest.as_deref()),
        secret.metadata.namespace.as_deref(),
    );
    let storage_name = secret.metadata.name.clone().unwrap_or_default();
    let summary = release_summary(
        cluster_context,
        HELM_STORAGE_SECRET,
        &storage_name,
        &secret.metadata,
        decoded.as_ref(),
    )?;
    Ok(HelmStorageRecord {
        summary,
        release,
        values_summary,
        manifest_summary,
    })
}

fn configmap_record(
    cluster_context: &str,
    configmap: &mut ConfigMap,
) -> Result<HelmStorageRecord, AppError> {
    let release_data = configmap
        .data
        .as_ref()
        .and_then(|data| data.get("release"))
        .map(|data| data.as_bytes());
    let decoded = release_data.and_then(decode_helm_release);
    let release = decoded.as_ref().and_then(safe_release_metadata);
    let values_summary =
        values_summary(decoded.as_ref().and_then(|release| release.config.as_ref()));
    let manifest_summary = manifest_summary(
        decoded
            .as_ref()
            .and_then(|release| release.manifest.as_deref()),
        configmap.metadata.namespace.as_deref(),
    );
    let storage_name = configmap.metadata.name.clone().unwrap_or_default();
    let summary = release_summary(
        cluster_context,
        HELM_STORAGE_CONFIGMAP,
        &storage_name,
        &configmap.metadata,
        decoded.as_ref(),
    )?;
    Ok(HelmStorageRecord {
        summary,
        release,
        values_summary,
        manifest_summary,
    })
}

fn release_summary(
    cluster_context: &str,
    storage_kind: &str,
    storage_name: &str,
    metadata: &ObjectMeta,
    decoded: Option<&DecodedHelmRelease>,
) -> Result<HelmReleaseSummary, AppError> {
    let labels = metadata.labels.as_ref();
    let namespace = decoded
        .and_then(|release| release.namespace.clone())
        .or_else(|| metadata.namespace.clone())
        .ok_or_else(|| AppError::new("Helm release storage object has no namespace", "cluster"))?;
    let name = decoded
        .and_then(|release| release.name.clone())
        .or_else(|| label_value(labels, "name"))
        .unwrap_or_else(|| release_name_from_storage_name(storage_name));
    let revision = decoded
        .and_then(|release| release.version)
        .or_else(|| label_value(labels, "version").and_then(|value| value.parse().ok()));
    let status = decoded
        .and_then(|release| release.info.as_ref())
        .and_then(|info| info.status.clone())
        .or_else(|| label_value(labels, "status"));
    let updated_at = decoded
        .and_then(|release| release.info.as_ref())
        .and_then(|info| info.last_deployed.clone())
        .filter(|value| !value.is_empty());
    let updated = updated_at.as_deref().and_then(parse_rfc3339);
    let created = metadata
        .creation_timestamp
        .as_ref()
        .and_then(|time| crate::commands::helpers::k8s_timestamp_to_datetime(&time.0));
    let chart_metadata = decoded
        .and_then(|release| release.chart.as_ref())
        .and_then(|chart| chart.metadata.as_ref());
    let chart = chart_metadata
        .and_then(|metadata| chart_label(metadata.name.as_deref(), metadata.version.as_deref()));
    let app_version = chart_metadata.and_then(|metadata| metadata.app_version.clone());

    Ok(HelmReleaseSummary {
        cluster: cluster_context.to_string(),
        name,
        namespace,
        age: resource_age(updated.or(created)),
        updated_at,
        created_at: k8s_creation_timestamp_to_rfc3339(&metadata.creation_timestamp),
        chart,
        app_version,
        revision,
        status,
        storage_kind: storage_kind.to_string(),
        storage_name: storage_name.to_string(),
    })
}

fn latest_releases(records: Vec<HelmStorageRecord>) -> Vec<HelmReleaseSummary> {
    let mut latest: BTreeMap<(String, String), HelmReleaseSummary> = BTreeMap::new();
    for record in records {
        let key = (
            record.summary.namespace.clone(),
            record.summary.name.clone(),
        );
        let replace = latest
            .get(&key)
            .map(|existing| record.summary.revision.unwrap_or(0) > existing.revision.unwrap_or(0))
            .unwrap_or(true);
        if replace {
            latest.insert(key, record.summary);
        }
    }
    latest.into_values().collect()
}

fn decode_helm_release(data: &[u8]) -> Option<DecodedHelmRelease> {
    let encoded = std::str::from_utf8(data).ok()?.trim();
    let gzipped = STANDARD.decode(encoded).ok()?;
    let mut decoder = GzDecoder::new(gzipped.as_slice());
    let mut json = String::new();
    decoder.read_to_string(&mut json).ok()?;
    serde_json::from_str(&json).ok()
}

fn safe_release_metadata(release: &DecodedHelmRelease) -> Option<serde_json::Value> {
    Some(serde_json::json!({
        "name": release.name,
        "namespace": release.namespace,
        "version": release.version,
        "info": release.info,
        "chart": release.chart,
    }))
}

fn values_summary(config: Option<&serde_json::Value>) -> HelmValuesSummary {
    match config {
        Some(serde_json::Value::Object(values)) => {
            let mut top_level_keys: Vec<String> = values.keys().cloned().collect();
            top_level_keys.sort();
            HelmValuesSummary {
                has_values: !values.is_empty(),
                value_count: values.len(),
                top_level_keys,
            }
        }
        Some(serde_json::Value::Null) => HelmValuesSummary {
            has_values: false,
            value_count: 0,
            top_level_keys: Vec::new(),
        },
        Some(_) => HelmValuesSummary {
            has_values: true,
            value_count: 1,
            top_level_keys: Vec::new(),
        },
        None => HelmValuesSummary {
            has_values: false,
            value_count: 0,
            top_level_keys: Vec::new(),
        },
    }
}

fn manifest_summary(
    manifest: Option<&str>,
    fallback_namespace: Option<&str>,
) -> HelmManifestSummary {
    const RESOURCE_PREVIEW_LIMIT: usize = 50;

    let Some(manifest) = manifest.filter(|value| !value.trim().is_empty()) else {
        return HelmManifestSummary {
            resource_count: 0,
            resources: Vec::new(),
            truncated: false,
        };
    };

    let mut resource_count = 0;
    let mut resources = Vec::new();
    for document in serde_yaml::Deserializer::from_str(manifest) {
        let Ok(value) = serde_yaml::Value::deserialize(document) else {
            continue;
        };
        let Some(resource) = manifest_resource_summary(&value, fallback_namespace) else {
            continue;
        };
        resource_count += 1;
        if resources.len() < RESOURCE_PREVIEW_LIMIT {
            resources.push(resource);
        }
    }

    HelmManifestSummary {
        resource_count,
        truncated: resource_count > resources.len(),
        resources,
    }
}

fn manifest_resource_summary(
    value: &serde_yaml::Value,
    _fallback_namespace: Option<&str>,
) -> Option<HelmManifestResourceSummary> {
    let mapping = value.as_mapping()?;
    let api_version = yaml_string(mapping, "apiVersion");
    let kind = yaml_string(mapping, "kind");
    let metadata = mapping
        .get(serde_yaml::Value::String("metadata".to_string()))
        .and_then(serde_yaml::Value::as_mapping);
    let name = metadata.and_then(|metadata| yaml_string(metadata, "name"));
    let namespace = metadata.and_then(|metadata| yaml_string(metadata, "namespace"));

    if api_version.is_none() && kind.is_none() && name.is_none() {
        return None;
    }

    Some(HelmManifestResourceSummary {
        api_version,
        kind,
        name,
        namespace,
    })
}

fn yaml_string(mapping: &serde_yaml::Mapping, key: &str) -> Option<String> {
    mapping
        .get(serde_yaml::Value::String(key.to_string()))
        .and_then(serde_yaml::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn label_value(labels: Option<&BTreeMap<String, String>>, key: &str) -> Option<String> {
    labels.and_then(|labels| labels.get(key).cloned())
}

fn is_helm_owned(labels: Option<&BTreeMap<String, String>>) -> bool {
    matches!(labels.and_then(|labels| labels.get("owner")), Some(owner) if owner == "helm")
}

fn release_name_from_storage_name(storage_name: &str) -> String {
    storage_name
        .strip_prefix("sh.helm.release.v1.")
        .and_then(|rest| rest.rsplit_once(".v"))
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| storage_name.to_string())
}

fn chart_label(name: Option<&str>, version: Option<&str>) -> Option<String> {
    match (name, version) {
        (Some(name), Some(version)) if !name.is_empty() && !version.is_empty() => {
            Some(format!("{name}-{version}"))
        }
        (Some(name), _) if !name.is_empty() => Some(name.to_string()),
        _ => None,
    }
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn redact_secret_release(secret: &mut Secret) {
    if let Some(data) = secret.data.as_mut() {
        if let Some(release) = data.get_mut("release") {
            release.0 = b"REDACTED".to_vec();
        }
    }
}

fn redact_configmap_release(configmap: &mut ConfigMap) {
    if let Some(data) = configmap.data.as_mut() {
        if let Some(release) = data.get_mut("release") {
            *release = "REDACTED".to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helm_owner_label_marks_release_storage() {
        let labels = BTreeMap::from([("owner".to_string(), "helm".to_string())]);

        assert!(is_helm_owned(Some(&labels)));
    }

    #[test]
    fn missing_helm_owner_label_rejects_storage() {
        let labels = BTreeMap::from([("app".to_string(), "not-helm".to_string())]);

        assert!(!is_helm_owned(Some(&labels)));
        assert!(!is_helm_owned(None));
    }

    #[test]
    fn values_summary_exposes_only_value_keys() {
        let config = serde_json::json!({
            "image": { "tag": "2026.5.22" },
            "replicaCount": 2,
        });

        let summary = values_summary(Some(&config));

        assert!(summary.has_values);
        assert_eq!(summary.value_count, 2);
        assert_eq!(summary.top_level_keys, vec!["image", "replicaCount"]);
    }

    #[test]
    fn values_summary_treats_explicit_null_as_empty() {
        let summary = values_summary(Some(&serde_json::Value::Null));

        assert!(!summary.has_values);
        assert_eq!(summary.value_count, 0);
        assert!(summary.top_level_keys.is_empty());
    }

    #[test]
    fn manifest_summary_extracts_resource_refs_without_manifest_body() {
        let manifest = r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: payments-api
---
apiVersion: v1
kind: Service
metadata:
  name: payments-api
  namespace: payments
"#;

        let summary = manifest_summary(Some(manifest), Some("default"));

        assert_eq!(summary.resource_count, 2);
        assert!(!summary.truncated);
        assert_eq!(summary.resources[0].kind.as_deref(), Some("Deployment"));
        assert_eq!(summary.resources[0].namespace, None);
        assert_eq!(summary.resources[1].kind.as_deref(), Some("Service"));
        assert_eq!(summary.resources[1].namespace.as_deref(), Some("payments"));
    }

    #[test]
    fn manifest_summary_preserves_missing_namespace_without_kind_guessing() {
        let manifest = r#"
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: payments-reader
---
apiVersion: networking.k8s.io/v1
kind: IngressClass
metadata:
  name: internal
"#;

        let summary = manifest_summary(Some(manifest), Some("payments"));

        assert_eq!(summary.resource_count, 2);
        assert_eq!(summary.resources[0].kind.as_deref(), Some("ClusterRole"));
        assert_eq!(summary.resources[0].namespace, None);
        assert_eq!(summary.resources[1].kind.as_deref(), Some("IngressClass"));
        assert_eq!(summary.resources[1].namespace, None);
    }
}
