//! List responses need release identity and chart metadata, never values or manifests.
use super::{
    decode_release, manifest_summary_from_resources, release_summary, values_summary,
    DecodedHelmChart, DecodedHelmInfo, DecodedHelmRelease, HelmStorageRecord,
    HELM_STORAGE_CONFIGMAP, HELM_STORAGE_SECRET,
};
use crate::models::AppError;
use k8s_openapi::{
    api::core::v1::{ConfigMap, Secret},
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
struct ReleaseSummary {
    name: Option<String>,
    namespace: Option<String>,
    version: Option<i32>,
    info: Option<DecodedHelmInfo>,
    chart: Option<DecodedHelmChart>,
    // Validate the original string type without allocating its contents.
    #[serde(rename = "manifest")]
    _manifest: Option<DiscardedString>,
}

struct DiscardedString;

impl<'de> Deserialize<'de> for DiscardedString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = DiscardedString;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a string")
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(DiscardedString)
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

fn record(
    cluster: &str,
    kind: &str,
    metadata: &ObjectMeta,
    data: Option<&[u8]>,
) -> Result<HelmStorageRecord, AppError> {
    let decoded = data.map(decode_release::<ReleaseSummary>).transpose();
    let (decoded, decode_error) = match decoded {
        Ok(decoded) => (decoded, None),
        Err(error) => (None, Some(error)),
    };
    let decoded = decoded.map(|value| DecodedHelmRelease {
        name: value.name,
        namespace: value.namespace,
        version: value.version,
        info: value.info,
        chart: value.chart,
        config: None,
        manifest: None,
    });
    Ok(HelmStorageRecord {
        summary: release_summary(
            cluster,
            kind,
            metadata.name.as_deref().unwrap_or_default(),
            metadata,
            decoded.as_ref(),
        )?,
        decode_error,
        release: None,
        values_summary: values_summary(None),
        manifest_summary: manifest_summary_from_resources(&[]),
        manifest_resources: Vec::new(),
    })
}

pub(super) fn secret_list_record(
    cluster: &str,
    secret: &mut Secret,
) -> Result<HelmStorageRecord, AppError> {
    record(
        cluster,
        HELM_STORAGE_SECRET,
        &secret.metadata,
        secret
            .data
            .as_ref()
            .and_then(|data| data.get("release"))
            .map(|data| data.0.as_slice()),
    )
}

pub(super) fn configmap_list_record(
    cluster: &str,
    config: &mut ConfigMap,
) -> Result<HelmStorageRecord, AppError> {
    record(
        cluster,
        HELM_STORAGE_CONFIGMAP,
        &config.metadata,
        config
            .data
            .as_ref()
            .and_then(|data| data.get("release"))
            .map(std::string::String::as_bytes),
    )
}
