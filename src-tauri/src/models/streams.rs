use serde::{Deserialize, Serialize};

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
    pub cluster_context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_env_var: Option<String>,
    pub namespace: String,
    pub pod_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_lines: Option<i64>,
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
    },
    Error {
        stream_id: String,
        message: String,
    },
    Stopped {
        stream_id: String,
    },
}
