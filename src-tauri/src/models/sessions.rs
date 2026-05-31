use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortForwardRequest {
    pub cluster_context: String,
    pub namespace: String,
    pub pod_name: String,
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
    pub pod_name: String,
    pub remote_port: u16,
    pub local_port: u16,
    pub local_address: String,
    pub local_url: String,
    pub status: String,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}
