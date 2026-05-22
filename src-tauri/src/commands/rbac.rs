use crate::commands::helpers::{k8s_creation_timestamp_to_rfc3339, list_params, resource_age};
use crate::models::{
    AppError, RbacBindingSummary, RbacInspectionSummary, RbacNamespaceAccessSummary,
    RbacRoleSummary, RbacRuleSummary, RbacSubjectSummary, ServiceAccountSummary,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::{
    core::v1::ServiceAccount,
    rbac::v1::{ClusterRole, ClusterRoleBinding, PolicyRule, Role, RoleBinding, Subject},
};
use kube::{api::Api, config::KubeConfigOptions, Client};
use std::collections::{BTreeMap, BTreeSet};

#[path = "rbac_risk.rs"]
mod rbac_risk;
use rbac_risk::{binding_risks, dedupe_risks, dedupe_subjects, rule_risks, service_account_risks};

#[cfg(test)]
#[path = "rbac_tests.rs"]
mod rbac_tests;

#[tauri::command]
pub async fn list_rbac_inspection(
    cluster_context: String,
    namespaces: Vec<String>,
) -> Result<RbacInspectionSummary, AppError> {
    rbac_inspection_from(cluster_context, namespaces).await
}

pub async fn rbac_inspection_from(
    cluster_context: String,
    namespaces: Vec<String>,
) -> Result<RbacInspectionSummary, AppError> {
    let client = client_for_context(&cluster_context).await?;
    let namespaces = clean_namespaces(namespaces);
    let service_accounts =
        list_service_accounts(client.clone(), &cluster_context, &namespaces).await?;
    let roles = list_roles(client.clone(), &cluster_context, &namespaces).await?;
    let role_bindings = list_role_bindings(client.clone(), &cluster_context, &namespaces).await?;
    let cluster_roles = list_cluster_roles(client.clone(), &cluster_context).await?;
    let cluster_role_bindings = list_cluster_role_bindings(client, &cluster_context).await?;
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
        service_accounts,
        roles,
        cluster_roles,
        role_bindings,
        cluster_role_bindings,
        namespace_access,
    })
}

async fn client_for_context(cluster_context: &str) -> Result<Client, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.to_string()),
        ..Default::default()
    };
    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;
    Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))
}

fn clean_namespaces(namespaces: Vec<String>) -> Vec<String> {
    namespaces
        .into_iter()
        .map(|namespace| namespace.trim().to_string())
        .filter(|namespace| !namespace.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

async fn list_service_accounts(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
) -> Result<Vec<ServiceAccountSummary>, AppError> {
    if namespaces.is_empty() {
        let api: Api<ServiceAccount> = Api::all(client);
        return api
            .list(&list_params())
            .await
            .map_err(AppError::from)
            .map(|items| {
                items
                    .items
                    .into_iter()
                    .map(|item| service_account_summary(cluster_context, item))
                    .collect()
            });
    }

    let mut summaries = Vec::new();
    for namespace in namespaces {
        let api: Api<ServiceAccount> = Api::namespaced(client.clone(), namespace);
        let items = api.list(&list_params()).await.map_err(AppError::from)?;
        summaries.extend(
            items
                .items
                .into_iter()
                .map(|item| service_account_summary(cluster_context, item)),
        );
    }
    Ok(summaries)
}

async fn list_roles(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
) -> Result<Vec<RbacRoleSummary>, AppError> {
    if namespaces.is_empty() {
        let api: Api<Role> = Api::all(client);
        return api
            .list(&list_params())
            .await
            .map_err(AppError::from)
            .map(|items| {
                items
                    .items
                    .into_iter()
                    .map(|item| role_summary(cluster_context, "Role", item.metadata, item.rules))
                    .collect()
            });
    }

    let mut summaries = Vec::new();
    for namespace in namespaces {
        let api: Api<Role> = Api::namespaced(client.clone(), namespace);
        let items = api.list(&list_params()).await.map_err(AppError::from)?;
        summaries.extend(
            items
                .items
                .into_iter()
                .map(|item| role_summary(cluster_context, "Role", item.metadata, item.rules)),
        );
    }
    Ok(summaries)
}

async fn list_cluster_roles(
    client: Client,
    cluster_context: &str,
) -> Result<Vec<RbacRoleSummary>, AppError> {
    let api: Api<ClusterRole> = Api::all(client);
    api.list(&list_params())
        .await
        .map_err(AppError::from)
        .map(|items| {
            items
                .items
                .into_iter()
                .map(|item| role_summary(cluster_context, "ClusterRole", item.metadata, item.rules))
                .collect()
        })
}

async fn list_role_bindings(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
) -> Result<Vec<RbacBindingSummary>, AppError> {
    if namespaces.is_empty() {
        let api: Api<RoleBinding> = Api::all(client);
        return api
            .list(&list_params())
            .await
            .map_err(AppError::from)
            .map(|items| {
                items
                    .items
                    .into_iter()
                    .map(|item| {
                        binding_summary(
                            cluster_context,
                            "RoleBinding",
                            item.metadata,
                            item.role_ref.kind,
                            item.role_ref.name,
                            item.subjects,
                        )
                    })
                    .collect()
            });
    }

    let mut summaries = Vec::new();
    for namespace in namespaces {
        let api: Api<RoleBinding> = Api::namespaced(client.clone(), namespace);
        let items = api.list(&list_params()).await.map_err(AppError::from)?;
        summaries.extend(items.items.into_iter().map(|item| {
            binding_summary(
                cluster_context,
                "RoleBinding",
                item.metadata,
                item.role_ref.kind,
                item.role_ref.name,
                item.subjects,
            )
        }));
    }
    Ok(summaries)
}

async fn list_cluster_role_bindings(
    client: Client,
    cluster_context: &str,
) -> Result<Vec<RbacBindingSummary>, AppError> {
    let api: Api<ClusterRoleBinding> = Api::all(client);
    api.list(&list_params())
        .await
        .map_err(AppError::from)
        .map(|items| {
            items
                .items
                .into_iter()
                .map(|item| {
                    binding_summary(
                        cluster_context,
                        "ClusterRoleBinding",
                        item.metadata,
                        item.role_ref.kind,
                        item.role_ref.name,
                        item.subjects,
                    )
                })
                .collect()
        })
}

fn service_account_summary(
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

fn role_summary(
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

fn binding_summary(
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
