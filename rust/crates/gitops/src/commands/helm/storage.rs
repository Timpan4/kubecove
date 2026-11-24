use super::manifest::{manifest_resources, manifest_summary_from_resources};
use super::redaction::redact_configmap_release;
use super::time::parse_rfc3339;
use super::values::values_summary;
use crate::commands::{
    ClusterAccess,
    helpers::{
        k8s_creation_timestamp_to_rfc3339, redact_secret, resource_age, serialize_resource_document,
    },
};
use crate::models::AppErrorKind;
use crate::models::{
    AppError, HelmManifestResourceSummary, HelmManifestSummary, HelmReleaseDetails,
    HelmReleaseSummary, HelmValuesSummary, YamlEncoding, YamlViewMode,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::read::GzDecoder;
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use kube::{
    Client,
    api::{Api, ObjectMeta},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read};

#[path = "storage_list.rs"]
mod list_projection;
#[cfg(feature = "bench-support")]
pub use list_projection::benchmark_storage_records;
pub use list_projection::list_helm_releases;

const HELM_STORAGE_SECRET: &str = "Secret";
const HELM_STORAGE_CONFIGMAP: &str = "ConfigMap";
// Kubernetes limits Secret and ConfigMap data to 1 MiB. The decoded budget is
// the separately approved application limit, including gzip expansion.
pub const MAX_STORED_RELEASE_BYTES: usize = 1024 * 1024;
pub const MAX_DECODED_RELEASE_BYTES: u64 = 32 * 1024 * 1024;

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

#[derive(Default)]
struct ReleaseFields<'a> {
    name: Option<&'a str>,
    namespace: Option<&'a str>,
    version: Option<i32>,
    info: Option<&'a DecodedHelmInfo>,
    chart: Option<&'a DecodedHelmChart>,
}

#[derive(Debug, Clone)]
pub(super) struct HelmStorageRecord {
    decode_error: Option<AppError>,
    pub(super) summary: HelmReleaseSummary,
    pub(super) release: Option<serde_json::Value>,
    pub(super) values_summary: HelmValuesSummary,
    pub(super) manifest_summary: HelmManifestSummary,
    pub(super) manifest_resources: Vec<HelmManifestResourceSummary>,
}

pub async fn get_helm_release_details(
    cluster_context: String,
    namespace: String,
    storage_kind: String,
    storage_name: String,
    kubeconfig_env_var: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
) -> Result<HelmReleaseDetails, AppError> {
    let client = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?
        .read_client(&cluster_context)
        .await?;

    let (record, metadata, yaml) = release_storage_object(
        client,
        &cluster_context,
        &namespace,
        &storage_kind,
        &storage_name,
        yaml_view_mode.unwrap_or_default(),
        yaml_encoding.unwrap_or_default(),
    )
    .await?;
    if let Some(error) = record.decode_error {
        return Err(error);
    }
    Ok(HelmReleaseDetails {
        summary: record.summary,
        yaml,
        metadata,
        values_summary: record.values_summary,
        manifest_summary: record.manifest_summary,
        release: record.release,
    })
}

pub(super) async fn get_helm_storage_record(
    client: Client,
    cluster_context: &str,
    namespace: &str,
    storage_kind: &str,
    storage_name: &str,
) -> Result<HelmStorageRecord, AppError> {
    let (record, _, _) = release_storage_object(
        client,
        cluster_context,
        namespace,
        storage_kind,
        storage_name,
        YamlViewMode::Kubectl,
        YamlEncoding::Yaml,
    )
    .await?;
    if let Some(error) = record.decode_error {
        return Err(error);
    }
    Ok(record)
}

async fn release_storage_object(
    client: Client,
    cluster_context: &str,
    namespace: &str,
    storage_kind: &str,
    storage_name: &str,
    yaml_view_mode: YamlViewMode,
    yaml_encoding: YamlEncoding,
) -> Result<(HelmStorageRecord, serde_json::Value, String), AppError> {
    crate::commands::helpers::validate_namespace(Some(namespace))?;
    crate::commands::helpers::validate_path_segment(storage_name, "Helm storage name")?;
    match storage_kind {
        HELM_STORAGE_SECRET => {
            let api: Api<Secret> = Api::namespaced(client, namespace);
            let mut secret = api.get(storage_name).await.map_err(AppError::from)?;
            if !is_helm_owned(secret.metadata.labels.as_ref()) {
                return Err(AppError::new(
                    "storage object is not a Helm release",
                    AppErrorKind::Validation,
                ));
            }
            let record = secret_record(cluster_context, &secret)?;
            redact_secret(&mut secret);
            let metadata = serde_json::to_value(&secret.metadata).map_err(|e| {
                AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e)
            })?;
            let yaml = serialize_resource_document(&secret, yaml_view_mode, yaml_encoding)?;
            Ok((record, metadata, yaml))
        }
        HELM_STORAGE_CONFIGMAP => {
            let api: Api<ConfigMap> = Api::namespaced(client, namespace);
            let mut configmap = api.get(storage_name).await.map_err(AppError::from)?;
            if !is_helm_owned(configmap.metadata.labels.as_ref()) {
                return Err(AppError::new(
                    "storage object is not a Helm release",
                    AppErrorKind::Validation,
                ));
            }
            let record = configmap_record(cluster_context, &configmap)?;
            let metadata = serde_json::to_value(&configmap.metadata).map_err(|e| {
                AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e)
            })?;
            redact_configmap_release(&mut configmap);
            let yaml = serialize_resource_document(&configmap, yaml_view_mode, yaml_encoding)?;
            Ok((record, metadata, yaml))
        }
        _ => Err(AppError::new(
            "unsupported Helm storage kind",
            AppErrorKind::Validation,
        )),
    }
}

fn secret_release_data(secret: &Secret) -> Option<&[u8]> {
    secret
        .data
        .as_ref()
        .and_then(|data| data.get("release"))
        .map(|data| data.0.as_slice())
}

fn configmap_release_data(configmap: &ConfigMap) -> Option<&[u8]> {
    configmap
        .data
        .as_ref()
        .and_then(|data| data.get("release"))
        .map(String::as_bytes)
}

fn secret_record(cluster_context: &str, secret: &Secret) -> Result<HelmStorageRecord, AppError> {
    selected_release_record(
        cluster_context,
        HELM_STORAGE_SECRET,
        &secret.metadata,
        secret_release_data(secret),
    )
}

fn configmap_record(
    cluster_context: &str,
    configmap: &ConfigMap,
) -> Result<HelmStorageRecord, AppError> {
    selected_release_record(
        cluster_context,
        HELM_STORAGE_CONFIGMAP,
        &configmap.metadata,
        configmap_release_data(configmap),
    )
}

fn selected_release_record(
    cluster_context: &str,
    storage_kind: &str,
    metadata: &ObjectMeta,
    release_data: Option<&[u8]>,
) -> Result<HelmStorageRecord, AppError> {
    let decoded = release_data.map(decode_helm_release).transpose();
    let (decoded, decode_error) = match decoded {
        Ok(decoded) => (decoded, None),
        Err(error) => (None, Some(error)),
    };
    let release = decoded.as_ref().and_then(safe_release_metadata);
    let values_summary =
        values_summary(decoded.as_ref().and_then(|release| release.config.as_ref()));
    let manifest_resources = manifest_resources(
        decoded
            .as_ref()
            .and_then(|release| release.manifest.as_deref()),
    );
    let manifest_summary = manifest_summary_from_resources(&manifest_resources);
    let fields = decoded.as_ref().map(|value| ReleaseFields {
        name: value.name.as_deref(),
        namespace: value.namespace.as_deref(),
        version: value.version,
        info: value.info.as_ref(),
        chart: value.chart.as_ref(),
    });
    let summary = release_summary(cluster_context, storage_kind, metadata, fields)?;
    Ok(HelmStorageRecord {
        decode_error,
        summary,
        release,
        values_summary,
        manifest_summary,
        manifest_resources,
    })
}

fn release_summary(
    cluster_context: &str,
    storage_kind: &str,
    metadata: &ObjectMeta,
    fields: Option<ReleaseFields<'_>>,
) -> Result<HelmReleaseSummary, AppError> {
    let ReleaseFields {
        name,
        namespace,
        version,
        info,
        chart,
    } = fields.unwrap_or_default();
    let labels = metadata.labels.as_ref();
    let storage_name = metadata.name.as_deref().unwrap_or_default();
    let namespace = namespace
        .map(str::to_owned)
        .or_else(|| metadata.namespace.clone())
        .ok_or_else(|| {
            AppError::new(
                "Helm release storage object has no namespace",
                AppErrorKind::Cluster,
            )
        })?;
    let name = name
        .map(str::to_owned)
        .or_else(|| label_value(labels, "name"))
        .unwrap_or_else(|| release_name_from_storage_name(storage_name));
    let revision =
        version.or_else(|| label_value(labels, "version").and_then(|value| value.parse().ok()));
    let status = info
        .and_then(|info| info.status.clone())
        .or_else(|| label_value(labels, "status"));
    let updated_at = info
        .and_then(|info| info.last_deployed.clone())
        .filter(|value| !value.is_empty());
    let updated = updated_at.as_deref().and_then(parse_rfc3339);
    let created = metadata
        .creation_timestamp
        .as_ref()
        .and_then(|time| crate::commands::helpers::k8s_timestamp_to_datetime(&time.0));
    let chart_metadata = chart.and_then(|chart| chart.metadata.as_ref());
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

#[cfg(feature = "bench-support")]
pub fn benchmark_invalid_storage_record_is_retained(secret: Secret) -> bool {
    secret_record("benchmark", &secret).is_ok_and(|record| record.decode_error.is_some())
}

#[cfg(feature = "bench-support")]
pub fn benchmark_storage_record_error(secret: Secret) -> Option<(AppErrorKind, String)> {
    secret_record("benchmark", &secret)
        .ok()
        .and_then(|record| record.decode_error.map(|error| (error.kind, error.message)))
}

#[cfg(feature = "bench-support")]
pub fn benchmark_storage_record_projection(
    secret: Secret,
) -> Result<
    (
        HelmReleaseSummary,
        HelmValuesSummary,
        HelmManifestSummary,
        Option<serde_json::Value>,
        bool,
    ),
    AppError,
> {
    let record = secret_record("benchmark", &secret)?;
    Ok((
        record.summary,
        record.values_summary,
        record.manifest_summary,
        record.release,
        record.decode_error.is_some(),
    ))
}

fn decode_helm_release(data: &[u8]) -> Result<DecodedHelmRelease, AppError> {
    decode_release(data)
}

fn decode_release<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, AppError> {
    if data.len() > MAX_STORED_RELEASE_BYTES {
        return Err(AppError::new(
            "Helm release storage exceeds the Kubernetes 1 MiB limit",
            AppErrorKind::Validation,
        ));
    }
    let encoded = std::str::from_utf8(data)
        .map_err(|error| {
            AppError::new(
                "Helm release encoding is not UTF-8",
                AppErrorKind::Serialization,
            )
            .with_source(error)
        })?
        .trim();
    let decoded = STANDARD.decode(encoded).map_err(|error| {
        AppError::new(
            "Helm release encoding is not base64",
            AppErrorKind::Serialization,
        )
        .with_source(error)
    })?;
    let json = if decoded.starts_with(&[0x1f, 0x8b, 0x08]) {
        let mut json = Vec::new();
        GzDecoder::new(decoded.as_slice())
            .take(MAX_DECODED_RELEASE_BYTES + 1)
            .read_to_end(&mut json)
            .map_err(|error| {
                AppError::new(
                    "Could not decompress Helm release",
                    AppErrorKind::Serialization,
                )
                .with_source(error)
            })?;
        if json.len() as u64 > MAX_DECODED_RELEASE_BYTES {
            return Err(AppError::new(
                "Decoded Helm release exceeds the 32 MiB limit",
                AppErrorKind::Validation,
            ));
        }
        json
    } else {
        // Helm also accepts its older, uncompressed JSON storage format.
        decoded
    };
    serde_json::from_slice(&json).map_err(|error| {
        AppError::new(
            "Helm release contains invalid JSON",
            AppErrorKind::Serialization,
        )
        .with_source(error)
    })
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
        .map_or_else(|| storage_name.to_string(), |(name, _)| name.to_string())
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

#[cfg(test)]
#[path = "storage_tests.rs"]
mod storage_tests;
