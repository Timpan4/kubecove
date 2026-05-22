use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum RbacRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RbacRiskIndicator {
    pub level: RbacRiskLevel,
    pub label: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RbacRuleSummary {
    pub verbs: Vec<String>,
    pub api_groups: Vec<String>,
    pub resources: Vec<String>,
    pub resource_names: Vec<String>,
    pub non_resource_urls: Vec<String>,
    pub risks: Vec<RbacRiskIndicator>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RbacSubjectSummary {
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAccountSummary {
    pub cluster: String,
    pub name: String,
    pub namespace: String,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automount_token: Option<bool>,
    pub secrets_count: usize,
    pub image_pull_secrets_count: usize,
    pub risks: Vec<RbacRiskIndicator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RbacRoleSummary {
    pub cluster: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub rules_count: usize,
    pub risks: Vec<RbacRiskIndicator>,
    pub rules: Vec<RbacRuleSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RbacBindingSummary {
    pub cluster: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub age: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub role_ref_kind: String,
    pub role_ref_name: String,
    pub subjects: Vec<RbacSubjectSummary>,
    pub risks: Vec<RbacRiskIndicator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RbacNamespaceAccessSummary {
    pub cluster: String,
    pub namespace: String,
    pub service_accounts: usize,
    pub roles: usize,
    pub role_bindings: usize,
    pub bound_subjects: Vec<RbacSubjectSummary>,
    pub risks: Vec<RbacRiskIndicator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RbacInspectionSummary {
    pub cluster: String,
    pub service_accounts: Vec<ServiceAccountSummary>,
    pub roles: Vec<RbacRoleSummary>,
    pub cluster_roles: Vec<RbacRoleSummary>,
    pub role_bindings: Vec<RbacBindingSummary>,
    pub cluster_role_bindings: Vec<RbacBindingSummary>,
    pub namespace_access: Vec<RbacNamespaceAccessSummary>,
}
