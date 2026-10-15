use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnectionSummary {
    pub server_url: String,
    pub server_version: String,
    pub session_grace_seconds: u64,
    pub connected: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSession {
    pub resume_token: String,
    pub session_grace_seconds: u64,
    pub server_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCredentials {
    pub server_url: String,
    pub bearer_token: String,
    pub resume_token: String,
}
