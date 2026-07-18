use crate::commands::{
    helpers::{k8s_creation_timestamp_to_rfc3339, resource_age},
    kubeconfig::KubeconfigSource,
};
use crate::models::{
    AppError, RbacBindingSummary, RbacCoverageStatus, RbacFamily, RbacFamilyCoverage,
    RbacInspectionSummary, RbacNamespaceAccessSummary, RbacRequestMode, RbacRiskIndicator,
    RbacRiskLevel, RbacRoleSummary, RbacRuleSummary, RbacSubjectSummary, ServiceAccountSummary,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::{
    core::v1::ServiceAccount,
    rbac::v1::{PolicyRule, Subject},
};
use kube::Client;
use std::collections::{BTreeMap, BTreeSet};
use tauri::State;

#[path = "rbac_risk.rs"]
mod rbac_risk;
use rbac_risk::{binding_risks, dedupe_risks, dedupe_subjects, rule_risks, service_account_risks};

#[path = "rbac_inventory.rs"]
mod rbac_inventory;
use rbac_inventory::{
    list_cluster_role_bindings, list_cluster_roles, list_role_bindings, list_roles,
    list_service_accounts, InventoryLoad,
};

#[cfg(test)]
#[path = "rbac_tests.rs"]
mod rbac_tests;

#[tauri::command]
pub async fn list_rbac_inspection(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    cancellations: State<'_, crate::commands::BackendCancellationRegistry>,
) -> Result<RbacInspectionSummary, AppError> {
    let cancellation = cancellations.register(cancel_scope, request_id);
    cancellation
        .run(rbac_inspection_from(cluster_context, kubeconfig_env_var))
        .await
}

pub async fn rbac_inspection_from(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<RbacInspectionSummary, AppError> {
    let client = client_for_context(&cluster_context, kubeconfig_env_var).await?;
    // RBAC is a cluster-security review; workspace namespace filters never narrow inventory.
    let namespaces = Vec::new();
    let (
        service_accounts_result,
        roles_result,
        cluster_roles_result,
        role_bindings_result,
        cluster_role_bindings_result,
    ) = tokio::join!(
        list_service_accounts(client.clone(), &cluster_context),
        list_roles(client.clone(), &cluster_context),
        list_cluster_roles(client.clone(), &cluster_context),
        list_role_bindings(client.clone(), &cluster_context),
        list_cluster_role_bindings(client, &cluster_context),
    );
    let mut warnings = Vec::new();
    let (service_accounts, service_accounts_coverage) = family(
        RbacFamily::ServiceAccounts,
        service_accounts_result,
        &mut warnings,
    );
    let (roles, roles_coverage) = family(RbacFamily::Roles, roles_result, &mut warnings);
    let (cluster_roles, cluster_roles_coverage) = family(
        RbacFamily::ClusterRoles,
        cluster_roles_result,
        &mut warnings,
    );
    let (mut role_bindings, role_bindings_coverage) = family(
        RbacFamily::RoleBindings,
        role_bindings_result,
        &mut warnings,
    );
    let (mut cluster_role_bindings, cluster_role_bindings_coverage) = family(
        RbacFamily::ClusterRoleBindings,
        cluster_role_bindings_result,
        &mut warnings,
    );
    inherit_binding_risks(&mut role_bindings, &roles, &cluster_roles);
    inherit_binding_risks(&mut cluster_role_bindings, &roles, &cluster_roles);
    let namespace_access = namespace_access_summary(
        &cluster_context,
        &namespaces,
        &service_accounts,
        &roles,
        &role_bindings,
        &cluster_role_bindings,
    );

    Ok(RbacInspectionSummary {
        cluster: cluster_context,
        refreshed_at: Utc::now().to_rfc3339(),
        warnings,
        coverage: vec![
            service_accounts_coverage,
            roles_coverage,
            cluster_roles_coverage,
            role_bindings_coverage,
            cluster_role_bindings_coverage,
        ],
        service_accounts,
        roles,
        cluster_roles,
        role_bindings,
        cluster_role_bindings,
        namespace_access,
    })
}

fn family<T>(
    family: RbacFamily,
    result: InventoryLoad<T>,
    warnings: &mut Vec<String>,
) -> (Vec<T>, RbacFamilyCoverage) {
    match result.error {
        None => {
            let coverage = coverage(
                family,
                result.items.len(),
                RbacCoverageStatus::Complete,
                None,
            );
            (result.items, coverage)
        }
        Some(error) => {
            let partial = !result.items.is_empty();
            let loaded_count = result.items.len();
            let message = format!(
                "{} {}: {}",
                family_label(family),
                if partial {
                    "partially loaded"
                } else {
                    "unavailable"
                },
                error.message
            );
            warnings.push(message.clone());
            let status = if partial {
                RbacCoverageStatus::Partial
            } else {
                RbacCoverageStatus::Unavailable
            };
            (
                result.items,
                coverage(family, loaded_count, status, Some(message)),
            )
        }
    }
}

fn coverage(
    family: RbacFamily,
    loaded_count: usize,
    status: RbacCoverageStatus,
    message: Option<String>,
) -> RbacFamilyCoverage {
    RbacFamilyCoverage {
        family,
        status,
        request_mode: if matches!(
            family,
            RbacFamily::ClusterRoles | RbacFamily::ClusterRoleBindings
        ) {
            RbacRequestMode::Cluster
        } else {
            RbacRequestMode::AllNamespaces
        },
        count: loaded_count,
        namespaces: Vec::new(),
        message,
    }
}

fn family_label(family: RbacFamily) -> &'static str {
    match family {
        RbacFamily::ServiceAccounts => "ServiceAccounts",
        RbacFamily::Roles => "Roles",
        RbacFamily::ClusterRoles => "ClusterRoles",
        RbacFamily::RoleBindings => "RoleBindings",
        RbacFamily::ClusterRoleBindings => "ClusterRoleBindings",
    }
}

fn inherit_binding_risks(
    bindings: &mut [RbacBindingSummary],
    roles: &[RbacRoleSummary],
    cluster_roles: &[RbacRoleSummary],
) {
    for binding in bindings {
        let referenced = match binding.role_ref_kind.as_str() {
            "Role" => roles.iter().find(|role| {
                role.name == binding.role_ref_name && role.namespace == binding.namespace
            }),
            "ClusterRole" => cluster_roles
                .iter()
                .find(|role| role.name == binding.role_ref_name),
            _ => None,
        };
        if let Some(role) = referenced {
            binding.risks.extend(role.risks.clone());
        } else {
            binding.risks.push(RbacRiskIndicator {
                level: RbacRiskLevel::Unknown,
                label: "Referenced role unavailable".to_string(),
                reason: "The referenced role was not loaded, so this binding cannot be classified as no flags.".to_string(),
            });
        }
        binding.risks = dedupe_risks(std::mem::take(&mut binding.risks));
    }
}

async fn client_for_context(
    cluster_context: &str,
    kubeconfig_env_var: Option<String>,
) -> Result<Client, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    source.client_for_context(cluster_context).await
}

pub(super) fn service_account_summary(
    cluster_context: &str,
    service_account: ServiceAccount,
) -> ServiceAccountSummary {
    let metadata = service_account.metadata;
    let secrets_count = service_account
        .secrets
        .as_ref()
        .map_or(0, std::vec::Vec::len);
    let image_pull_secrets_count = service_account
        .image_pull_secrets
        .as_ref()
        .map_or(0, std::vec::Vec::len);
    ServiceAccountSummary {
        cluster: cluster_context.to_string(),
        name: metadata.name.clone().unwrap_or_default(),
        namespace: metadata.namespace.clone().unwrap_or_default(),
        age: resource_age(k8s_time_to_datetime(&metadata.creation_timestamp)),
        created_at: k8s_creation_timestamp_to_rfc3339(&metadata.creation_timestamp),
        automount_token: service_account.automount_service_account_token,
        secrets_count,
        image_pull_secrets_count,
        risks: service_account_risks(service_account.automount_service_account_token),
    }
}

pub(super) fn role_summary(
    cluster_context: &str,
    kind: &str,
    metadata: kube::api::ObjectMeta,
    rules: Option<Vec<PolicyRule>>,
) -> RbacRoleSummary {
    let rule_summaries: Vec<RbacRuleSummary> = rules
        .unwrap_or_default()
        .into_iter()
        .map(rule_summary)
        .collect();
    let risks = dedupe_risks(
        rule_summaries
            .iter()
            .flat_map(|rule| rule.risks.clone())
            .collect(),
    );
    RbacRoleSummary {
        cluster: cluster_context.to_string(),
        kind: kind.to_string(),
        name: metadata.name.clone().unwrap_or_default(),
        namespace: metadata.namespace.clone(),
        age: resource_age(k8s_time_to_datetime(&metadata.creation_timestamp)),
        created_at: k8s_creation_timestamp_to_rfc3339(&metadata.creation_timestamp),
        rules_count: rule_summaries.len(),
        risks,
        rules: rule_summaries,
    }
}

pub(super) fn binding_summary(
    cluster_context: &str,
    kind: &str,
    metadata: kube::api::ObjectMeta,
    role_ref_kind: String,
    role_ref_name: String,
    subjects: Option<Vec<Subject>>,
) -> RbacBindingSummary {
    let binding_namespace = metadata.namespace.clone();
    let subject_summaries = subjects
        .unwrap_or_default()
        .into_iter()
        .map(|subject| subject_summary(subject, binding_namespace.as_deref()))
        .collect::<Vec<_>>();
    let risks = binding_risks(&role_ref_kind, &role_ref_name, &subject_summaries);
    RbacBindingSummary {
        cluster: cluster_context.to_string(),
        kind: kind.to_string(),
        name: metadata.name.clone().unwrap_or_default(),
        namespace: metadata.namespace.clone(),
        age: resource_age(k8s_time_to_datetime(&metadata.creation_timestamp)),
        created_at: k8s_creation_timestamp_to_rfc3339(&metadata.creation_timestamp),
        role_ref_kind,
        role_ref_name,
        subjects: subject_summaries,
        risks,
    }
}

fn subject_summary(subject: Subject, default_namespace: Option<&str>) -> RbacSubjectSummary {
    let namespace = if subject.kind == "ServiceAccount" && subject.namespace.is_none() {
        default_namespace.map(str::to_string)
    } else {
        subject.namespace
    };
    RbacSubjectSummary {
        kind: subject.kind,
        name: subject.name,
        namespace,
    }
}

fn rule_summary(rule: PolicyRule) -> RbacRuleSummary {
    let verbs = normalize_values(rule.verbs);
    let api_groups = normalize_values(rule.api_groups.unwrap_or_default());
    let resources = normalize_values(rule.resources.unwrap_or_default());
    let resource_names = normalize_values(rule.resource_names.unwrap_or_default());
    let non_resource_urls = normalize_values(rule.non_resource_urls.unwrap_or_default());
    let risks = rule_risks(&verbs, &resources);
    RbacRuleSummary {
        verbs,
        api_groups,
        resources,
        resource_names,
        non_resource_urls,
        risks,
    }
}

fn normalize_values(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn namespace_access_summary(
    cluster_context: &str,
    namespaces: &[String],
    service_accounts: &[ServiceAccountSummary],
    roles: &[RbacRoleSummary],
    role_bindings: &[RbacBindingSummary],
    cluster_role_bindings: &[RbacBindingSummary],
) -> Vec<RbacNamespaceAccessSummary> {
    let mut by_namespace: BTreeMap<String, RbacNamespaceAccessSummary> = BTreeMap::new();
    let scoped_namespaces = namespaces.iter().cloned().collect::<BTreeSet<_>>();

    for service_account in service_accounts {
        let entry = namespace_entry(
            &mut by_namespace,
            cluster_context,
            &service_account.namespace,
        );
        entry.service_accounts += 1;
        entry.risks.extend(service_account.risks.clone());
    }
    for role in roles {
        if let Some(namespace) = role.namespace.as_deref() {
            let entry = namespace_entry(&mut by_namespace, cluster_context, namespace);
            entry.roles += 1;
            entry.risks.extend(role.risks.clone());
        }
    }
    for binding in role_bindings {
        if let Some(namespace) = binding.namespace.as_deref() {
            let entry = namespace_entry(&mut by_namespace, cluster_context, namespace);
            entry.role_bindings += 1;
            entry.bound_subjects.extend(binding.subjects.clone());
            entry.risks.extend(binding.risks.clone());
        }
    }
    for binding in cluster_role_bindings {
        let mut binding_namespaces: BTreeMap<String, Vec<RbacSubjectSummary>> = BTreeMap::new();
        let mut cluster_wide_subjects = Vec::new();
        for subject in &binding.subjects {
            if subject.kind == "ServiceAccount" {
                if let Some(namespace) = subject.namespace.as_deref() {
                    if !scoped_namespaces.is_empty() && !scoped_namespaces.contains(namespace) {
                        continue;
                    }
                    binding_namespaces
                        .entry(namespace.to_string())
                        .or_default()
                        .push(subject.clone());
                }
            } else {
                cluster_wide_subjects.push(subject.clone());
            }
        }
        if !cluster_wide_subjects.is_empty() {
            let target_namespaces = if scoped_namespaces.is_empty() {
                by_namespace.keys().cloned().collect::<Vec<_>>()
            } else {
                namespaces.to_vec()
            };
            for namespace in target_namespaces {
                binding_namespaces
                    .entry(namespace)
                    .or_default()
                    .extend(cluster_wide_subjects.clone());
            }
        }
        for (namespace, subjects) in binding_namespaces {
            let entry = namespace_entry(&mut by_namespace, cluster_context, &namespace);
            entry.role_bindings += 1;
            entry.bound_subjects.extend(subjects);
            entry.risks.extend(binding.risks.clone());
        }
    }

    by_namespace
        .into_values()
        .map(|mut summary| {
            summary.bound_subjects = dedupe_subjects(summary.bound_subjects);
            summary.risks = dedupe_risks(summary.risks);
            summary
        })
        .collect()
}

fn namespace_entry<'a>(
    by_namespace: &'a mut BTreeMap<String, RbacNamespaceAccessSummary>,
    cluster_context: &str,
    namespace: &str,
) -> &'a mut RbacNamespaceAccessSummary {
    by_namespace
        .entry(namespace.to_string())
        .or_insert_with(|| RbacNamespaceAccessSummary {
            cluster: cluster_context.to_string(),
            namespace: namespace.to_string(),
            service_accounts: 0,
            roles: 0,
            role_bindings: 0,
            bound_subjects: Vec::new(),
            risks: Vec::new(),
        })
}

fn k8s_time_to_datetime(
    timestamp: &Option<k8s_openapi::apimachinery::pkg::apis::meta::v1::Time>,
) -> Option<DateTime<Utc>> {
    timestamp
        .as_ref()
        .and_then(|time| crate::commands::helpers::k8s_timestamp_to_datetime(&time.0))
}
