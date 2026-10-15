use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEventSummary {
    pub event_type: String,
    pub reason: String,
    pub message: String,
    pub count: i32,
    pub last_seen: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<String>,
    pub source: String,
    pub namespace: Option<String>,
}
