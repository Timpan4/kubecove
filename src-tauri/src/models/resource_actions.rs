use super::ClusterOperationTarget;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceActionKind {
    Edit,
    Restart,
    Pause,
    Resume,
    Rollback,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceActionStateRequest {
    #[serde(flatten)]
    pub target: ClusterOperationTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_env_var: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceActionRequest {
    #[serde(flatten)]
    pub target: ClusterOperationTarget,
    pub action: ResourceActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<ResourceActionEdits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_resource_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision_uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision_resource_version: Option<String>,
    pub confirmed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_env_var: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceActionEdits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,
    #[serde(default)]
    pub containers: Vec<ResourceContainerEdit>,
    #[serde(default)]
    pub labels: BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub annotations: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContainerEdit {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default)]
    pub env: Vec<ResourceEnvEdit>,
    #[serde(default)]
    pub requests: BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub limits: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEnvEdit {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceActionState {
    pub target: ClusterOperationTarget,
    pub uid: String,
    pub resource_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused: Option<bool>,
    pub containers: Vec<ResourceActionContainer>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub permissions: Vec<ResourceActionPermission>,
    pub revisions: Vec<ResourceActionRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceActionContainer {
    pub name: String,
    pub image: String,
    pub env: Vec<ResourceActionEnv>,
    pub requests: BTreeMap<String, String>,
    pub limits: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceActionEnv {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub referenced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceActionPermission {
    pub action: ResourceActionKind,
    pub allowed: bool,
    pub reason: String,
    pub required_permission: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceActionRevision {
    pub revision: i64,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceActionPreview {
    pub target: ClusterOperationTarget,
    pub effect: String,
    pub before_yaml: String,
    pub after_yaml: String,
    pub uid: String,
    pub resource_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_resource_version: Option<String>,
}
