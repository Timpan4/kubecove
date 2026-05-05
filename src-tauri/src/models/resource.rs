use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSummary {
    pub cluster_context: String,
    pub api_version: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restarts: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argo_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub helm_release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDetails {
    pub summary: ResourceSummary,
    pub yaml: String,
    pub metadata: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Value>,
}
