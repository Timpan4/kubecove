use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationSourceSummary {
    pub repo_url: Option<String>,
    pub target_revision: Option<String>,
    pub resolved_revision: Option<String>,
    pub path: Option<String>,
    pub chart: Option<String>,
    pub source_mode: Option<String>,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationSummary {
    pub cluster: String,
    pub name: String,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub namespace: Option<String>,
    pub project: Option<String>,
    pub sync_status: Option<String>,
    pub health_status: Option<String>,
    pub destination_namespace: Option<String>,
    pub destination_server: Option<String>,
    pub source_repo: Option<String>,
    pub source_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_count: Option<usize>,
    #[serde(default)]
    pub sources: Vec<ArgoApplicationSourceSummary>,
    #[serde(default)]
    pub resource_namespaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracked_resource_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationDetails {
    pub summary: ArgoApplicationSummary,
    pub yaml: String,
    pub metadata: serde_json::Value,
    pub status: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationSetSummary {
    pub cluster: String,
    pub name: String,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub namespace: Option<String>,
    pub project: Option<String>,
    pub status: Option<String>,
    pub sync_status: Option<String>,
    pub health_status: Option<String>,
    pub destination_namespace: Option<String>,
    pub destination_server: Option<String>,
    pub source_repo: Option<String>,
    pub source_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoApplicationSetDetails {
    pub summary: ArgoApplicationSetSummary,
    pub yaml: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoAppProjectSummary {
    pub cluster: String,
    pub name: String,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub namespace: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoAppProjectDetails {
    pub summary: ArgoAppProjectSummary,
    pub yaml: String,
    pub metadata: serde_json::Value,
}
