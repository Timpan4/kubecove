use serde::{Deserialize, Serialize};

use super::{ResourceEventSummary, ResourceListRequest, ResourceSummary};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IncidentSeverity {
    Degraded,
    Attention,
    Restarted,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentSignalSummary {
    pub kind: String,
    pub label: String,
    pub message: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentCockpitItem {
    pub resource: ResourceSummary,
    pub severity: IncidentSeverity,
    pub signals: Vec<IncidentSignalSummary>,
    pub warning_event_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_signal_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_warning_event: Option<ResourceEventSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentCockpitSummary {
    pub cluster: String,
    pub generated_at: String,
    pub requested_scope: Vec<ResourceListRequest>,
    pub items: Vec<IncidentCockpitItem>,
    pub warnings: Vec<String>,
}
