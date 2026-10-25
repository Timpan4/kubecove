use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnectionSummary {
    pub server_url: String,
    pub server_version: String,
    pub session_grace_seconds: u64,
    pub connected: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSession {
    pub resume_token: String,
    pub session_grace_seconds: u64,
    pub server_version: String,
}

impl std::fmt::Debug for ServerSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServerSession")
            .field("resume_token", &"[REDACTED]")
            .field("session_grace_seconds", &self.session_grace_seconds)
            .field("server_version", &self.server_version)
            .finish()
    }
}

pub struct RemoteCredentials {
    pub server_url: String,
    pub bearer_token: String,
    pub resume_token: String,
}

impl std::fmt::Debug for RemoteCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RemoteCredentials")
            .field("server_url", &self.server_url)
            .field("bearer_token", &"[REDACTED]")
            .field("resume_token", &"[REDACTED]")
            .finish()
    }
}
