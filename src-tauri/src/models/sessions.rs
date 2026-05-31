use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortForwardRequest {
    pub cluster_context: String,
    pub namespace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pod_name: Option<String>,
    pub remote_port: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_port: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortForwardSessionSummary {
    pub id: String,
    pub cluster_context: String,
    pub namespace: String,
    pub target_kind: String,
    pub target_name: String,
    pub pod_name: String,
    pub remote_port: u16,
    pub resolved_pod_name: String,
    pub resolved_pod_port: u16,
    pub local_port: u16,
    pub local_address: String,
    pub local_url: String,
    pub status: String,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}
