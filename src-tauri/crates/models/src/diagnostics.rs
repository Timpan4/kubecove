use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendDiagnosticStatus {
    Ok,
    Error,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendDiagnosticField {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendDiagnosticEvent {
    pub id: u64,
    pub recorded_at: String,
    pub command: String,
    pub status: BackendDiagnosticStatus,
    pub duration_ms: u64,
    pub summary: Vec<BackendDiagnosticField>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendCacheDiagnosticSnapshot {
    pub label: String,
    pub hits: u64,
    pub misses: u64,
    pub joins: u64,
    pub evictions: u64,
    pub restored_reload_failures: u64,
    pub ready: usize,
    pub dirty: usize,
    pub loading: usize,
    pub retained_items: usize,
    pub shallow_payload_bytes: usize,
    pub weight_kind: String,
}
