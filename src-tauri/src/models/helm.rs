use super::resource::ResourceSummary;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmReleaseSummary {
    pub cluster: String,
    pub name: String,
    pub namespace: String,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub storage_kind: String,
    pub storage_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmReleaseDetails {
    pub summary: HelmReleaseSummary,
    pub yaml: String,
    pub metadata: serde_json::Value,
    pub values_summary: HelmValuesSummary,
    pub manifest_summary: HelmManifestSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmReleaseReconciliation {
    pub summary: HelmReleaseSummary,
    pub totals: HelmReconciliationTotals,
    pub resources: Vec<HelmReconciliationResource>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmReconciliationTotals {
    pub tracked: usize,
    pub unlabeled_live: usize,
    pub missing: usize,
    pub label_only: usize,
    pub unavailable: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HelmReconciliationStatus {
    Tracked,
    UnlabeledLive,
    Missing,
    LabelOnly,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmReconciliationResource {
    pub api_version: Option<String>,
    pub kind: Option<String>,
    pub namespace: Option<String>,
    pub name: Option<String>,
    pub status: HelmReconciliationStatus,
    pub status_message: String,
    pub in_manifest: bool,
    pub explicit_helm_label: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_resource: Option<ResourceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmValuesSummary {
    pub has_values: bool,
    pub top_level_keys: Vec<String>,
    pub value_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmManifestSummary {
    pub resource_count: usize,
    pub resources: Vec<HelmManifestResourceSummary>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmManifestResourceSummary {
    pub api_version: Option<String>,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub namespace: Option<String>,
}
