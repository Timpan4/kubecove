use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerReferenceSummary {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub uid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSummary {
    pub kind: String,
    pub cluster: String,
    pub name: String,
    pub namespace: Option<String>,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plural: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespaced: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restarts: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argo_app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helm_release: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDetails {
    pub kind: String,
    pub cluster: String,
    pub name: String,
    pub namespace: Option<String>,
    pub yaml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDetailsFull {
    pub summary: ResourceSummary,
    pub yaml: String,
    pub metadata: serde_json::Value,
    pub status: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TopologyRelation {
    Owns,
    Creates,
    Groups,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub status: Option<String>,
    pub health: String,
    pub selectable: bool,
    pub summary: ResourceSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: TopologyRelation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTopology {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    pub warnings: Vec<String>,
}
