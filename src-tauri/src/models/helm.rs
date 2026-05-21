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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<serde_json::Value>,
}
