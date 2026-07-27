use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterOperationTarget {
    pub cluster_context: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleWorkloadRequest {
    #[serde(flatten)]
    pub target: ClusterOperationTarget,
    pub replicas: i32,
    pub confirmed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_env_var: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloutRestartRequest {
    #[serde(flatten)]
    pub target: ClusterOperationTarget,
    pub confirmed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_env_var: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResourceRequest {
    #[serde(flatten)]
    pub target: ClusterOperationTarget,
    pub confirmed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_env_var: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterOperationPreview {
    pub target: ClusterOperationTarget,
    pub effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterOperationResult {
    pub target: ClusterOperationTarget,
    pub effect: String,
}
