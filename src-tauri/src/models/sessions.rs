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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodExecConfirmation {
    pub acknowledged: bool,
    pub target: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodExecTerminalSize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodExecSessionRequest {
    pub cluster_context: String,
    pub namespace: String,
    pub pod_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    pub command: Vec<String>,
    pub stdin: bool,
    pub tty: bool,
    pub terminal_size: PodExecTerminalSize,
    pub confirmation: PodExecConfirmation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodExecSessionSummary {
    pub id: String,
    pub cluster_context: String,
    pub namespace: String,
    pub pod_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    pub command: Vec<String>,
    pub stdin: bool,
    pub tty: bool,
    pub terminal_cols: u16,
    pub terminal_rows: u16,
    pub status: String,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PodExecSessionMessage {
    Started {
        session_id: String,
        summary: PodExecSessionSummary,
    },
    Status {
        session_id: String,
        status: String,
        message: String,
    },
    Output {
        session_id: String,
        stream: String,
        data: String,
    },
    Error {
        session_id: String,
        message: String,
    },
    Exited {
        session_id: String,
        exit_code: Option<i32>,
        reason: Option<String>,
        message: Option<String>,
    },
    Stopped {
        session_id: String,
    },
}
