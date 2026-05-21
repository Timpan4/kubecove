use crate::commands::helpers::{k8s_creation_timestamp_to_rfc3339, list_params, resource_age};
use crate::models::{AppError, HelmReleaseDetails, HelmReleaseSummary};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use kube::{
    api::{Api, ListParams, ObjectMeta},
    config::KubeConfigOptions,
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
}

#[tauri::command]
pub async fn list_helm_releases(
    cluster_context: String,
) -> Result<Vec<HelmReleaseSummary>, AppError> {
    let client = client_for_context(&cluster_context).await?;
    let mut records = Vec::new();
    let mut errors = Vec::new();

    match list_secret_releases(client.clone(), &cluster_context).await {
        Ok(mut releases) => records.append(&mut releases),
        Err(err) => errors.push(err.message),
    }

    match list_configmap_releases(client, &cluster_context).await {
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
) -> Result<HelmReleaseDetails, AppError> {
    let client = client_for_context(&cluster_context).await?;

    match storage_kind.as_str() {
        HELM_STORAGE_SECRET => {
            let api: Api<Secret> = Api::namespaced(client, &namespace);
            let mut secret = api.get(&storage_name).await.map_err(AppError::from)?;
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
                release: record.release,
            })
        }
        HELM_STORAGE_CONFIGMAP => {
            let api: Api<ConfigMap> = Api::namespaced(client, &namespace);
            let mut configmap = api.get(&storage_name).await.map_err(AppError::from)?;
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
                release: record.release,
            })
        }
        _ => Err(AppError::new("unsupported Helm storage kind", "validation")),
    }
}

async fn client_for_context(cluster_context: &str) -> Result<Client, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.to_string()),
        ..Default::default()
    };
    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;
    Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))
}

async fn list_secret_releases(
    client: Client,
    cluster_context: &str,
) -> Result<Vec<HelmStorageRecord>, AppError> {
    let api: Api<Secret> = Api::all(client);
    let params = helm_list_params();
    let items = api.list(&params).await.map_err(AppError::from)?;
    items
        .items
        .into_iter()
        .map(|mut secret| secret_record(cluster_context, &mut secret))
        .collect()
}

async fn list_configmap_releases(
    client: Client,
    cluster_context: &str,
) -> Result<Vec<HelmStorageRecord>, AppError> {
    let api: Api<ConfigMap> = Api::all(client);
    let params = helm_list_params();
    let items = api.list(&params).await.map_err(AppError::from)?;
    items
        .items
        .into_iter()
        .map(|mut configmap| configmap_record(cluster_context, &mut configmap))
        .collect()
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
    let release = decoded
        .as_ref()
        .and_then(|release| serde_json::to_value(release).ok());
    let storage_name = secret.metadata.name.clone().unwrap_or_default();
    let summary = release_summary(
        cluster_context,
        HELM_STORAGE_SECRET,
        &storage_name,
        &secret.metadata,
        decoded.as_ref(),
    )?;
    Ok(HelmStorageRecord { summary, release })
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
    let release = decoded
        .as_ref()
        .and_then(|release| serde_json::to_value(release).ok());
    let storage_name = configmap.metadata.name.clone().unwrap_or_default();
    let summary = release_summary(
        cluster_context,
        HELM_STORAGE_CONFIGMAP,
        &storage_name,
        &configmap.metadata,
        decoded.as_ref(),
    )?;
    Ok(HelmStorageRecord { summary, release })
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

fn label_value(labels: Option<&BTreeMap<String, String>>, key: &str) -> Option<String> {
    labels.and_then(|labels| labels.get(key).cloned())
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
