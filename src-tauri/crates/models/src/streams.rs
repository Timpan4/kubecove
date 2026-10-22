use serde::{Deserialize, Serialize};

// Existing desktop remote WebSocket message and frame limit.
pub const MAX_WEBSOCKET_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
// Channel JSON wrapper with the longest u64 channel id, excluding its payload.
pub const CHANNEL_MESSAGE_ENVELOPE_BYTES: usize =
    b"{\"channelId\":18446744073709551615,\"message\":,\"type\":\"channel\"}".len();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCacheRefreshResult {
    pub cleared_entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchResourceKind {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plural: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespaced: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchResourceKey {
    pub resource_kind: WatchResourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodLogStreamRequest {
    #[serde(default)]
    pub batch_lines: bool,
    pub cluster_context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_env_var: Option<String>,
    pub namespace: String,
    pub pod_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_lines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedLogStreamRequest {
    #[serde(default)]
    pub batch_lines: bool,
    pub cluster_context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_env_var: Option<String>,
    pub namespace: String,
    pub target_kind: String,
    pub target_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_lines: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLineSource {
    pub pod_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchResourceTarget {
    pub cluster: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StreamMessage {
    Started {
        stream_id: String,
        label: String,
    },
    Status {
        stream_id: String,
        status: String,
        message: String,
    },
    ResourceChanged {
        stream_id: String,
        target: WatchResourceTarget,
        action: String,
    },
    ResourceEventsChanged {
        stream_id: String,
        target: WatchResourceTarget,
        action: String,
    },
    LogLine {
        stream_id: String,
        line: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<LogLineSource>,
    },
    LogLines {
        stream_id: String,
        lines: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<LogLineSource>,
    },
    Error {
        stream_id: String,
        message: String,
    },
    Stopped {
        stream_id: String,
    },
}
