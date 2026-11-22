use serde::{Deserialize, Serialize};

/// The desktop MCP listener's settings and state (ADR 0028).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub enabled: bool,
    pub port: u16,
    pub running: bool,
    /// The endpoint agents connect to, while running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Why the listener is not running although enabled, such as a port in use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The tools the endpoint serves.
    pub tools: Vec<String>,
}
