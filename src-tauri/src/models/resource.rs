use serde::{Deserialize, Serialize};

use super::DiscoveredResourceKind;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceHealth {
    Healthy,
    Attention,
    Degraded,
    Restarted,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerReferenceSummary {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub uid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitOpsOwnerSummary {
    pub provider: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub confidence: String,
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
    #[serde(default)]
    pub health: ResourceHealth,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_ops_owner: Option<GitOpsOwnerSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_kind: Option<DiscoveredResourceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum YamlViewMode {
    #[default]
    Kubectl,
    ApplyClean,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum YamlEncoding {
    #[default]
    Yaml,
    Kyaml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlApplyRequest {
    pub cluster_context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kubeconfig_env_var: Option<String>,
    pub kind: String,
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
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub yaml: String,
    #[serde(default)]
    pub yaml_encoding: YamlEncoding,
    #[serde(default)]
    pub force_conflicts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlApplyTarget {
    pub cluster_context: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlApplyPreview {
    pub target: YamlApplyTarget,
    pub current_yaml: String,
    pub dry_run_yaml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YamlApplyResult {
    pub target: YamlApplyTarget,
    pub applied_yaml: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KubernetesYamlLintSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesYamlLintDiagnostic {
    pub severity: KubernetesYamlLintSeverity,
    pub source: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesYamlLintResult {
    pub diagnostics: Vec<KubernetesYamlLintDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TopologyRelation {
    Owns,
    Creates,
    Groups,
    RoutesTo,
    Selects,
    Targets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub status: Option<String>,
    pub health: ResourceHealth,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub port_hints: Vec<String>,
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
