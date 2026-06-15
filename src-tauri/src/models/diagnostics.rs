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
