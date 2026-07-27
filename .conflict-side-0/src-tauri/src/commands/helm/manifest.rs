use crate::models::{HelmManifestResourceSummary, HelmManifestSummary};
use serde::Deserialize;

#[cfg(test)]
pub(super) fn manifest_summary(
    manifest: Option<&str>,
    _fallback_namespace: Option<&str>,
) -> HelmManifestSummary {
    manifest_summary_from_resources(&manifest_resources(manifest))
}

pub(crate) fn manifest_summary_from_resources(
    manifest_resources: &[HelmManifestResourceSummary],
) -> HelmManifestSummary {
    const RESOURCE_PREVIEW_LIMIT: usize = 50;

    HelmManifestSummary {
        resource_count: manifest_resources.len(),
        resources: manifest_resources
            .iter()
            .take(RESOURCE_PREVIEW_LIMIT)
            .cloned()
            .collect(),
        truncated: manifest_resources.len() > RESOURCE_PREVIEW_LIMIT,
    }
}

pub(crate) fn manifest_resources(manifest: Option<&str>) -> Vec<HelmManifestResourceSummary> {
    let Some(manifest) = manifest.filter(|value| !value.trim().is_empty()) else {
        return Vec::new();
    };

    let mut resources = Vec::new();
    for document in serde_yaml::Deserializer::from_str(manifest) {
        let Ok(value) = serde_yaml::Value::deserialize(document) else {
            continue;
        };
        let Some(resource) = manifest_resource_summary(&value) else {
            continue;
        };
        resources.push(resource);
    }
    resources
}

fn manifest_resource_summary(value: &serde_yaml::Value) -> Option<HelmManifestResourceSummary> {
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
