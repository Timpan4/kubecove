use k8s_openapi::api::{
    core::v1::{LocalObjectReference, ObjectReference, ServiceAccount},
    rbac::v1::{ClusterRole, ClusterRoleBinding, PolicyRule, Role, RoleBinding, RoleRef, Subject},
};
use kube::api::ObjectMeta;

use crate::{
    commands::rbac::benchmark_rbac_inventory_projection,
    models::{RbacCoverageStatus, RbacInspectionSummary},
};

#[derive(Clone)]
pub struct RbacInventoryInputs {
    pub namespaces: Vec<String>,
    pub service_accounts: Vec<ServiceAccount>,
    pub roles: Vec<Role>,
    pub cluster_roles: Vec<ClusterRole>,
    pub role_bindings: Vec<RoleBinding>,
    pub cluster_role_bindings: Vec<ClusterRoleBinding>,
}

pub fn inventory_inputs() -> RbacInventoryInputs {
    // Production passes no explicit namespace filter; rollups infer buckets from inventory.
    let namespaces = Vec::new();
    let service_accounts = (0..500)
        .map(|index| ServiceAccount {
            metadata: namespaced_metadata(&service_account_name(index), index),
            automount_service_account_token: Some(index % 5 == 0),
            secrets: Some(vec![ObjectReference {
                name: Some(format!("sa-token-{index:03}")),
                ..Default::default()
            }]),
            image_pull_secrets: Some(vec![LocalObjectReference {
                name: "registry-credentials".to_string(),
            }]),
        })
        .collect();
    let roles = (0..500)
        .map(|index| Role {
            metadata: namespaced_metadata(&role_name(index), index),
            rules: Some(rules(index)),
        })
        .collect();
    let cluster_roles = (0..500)
        .map(|index| ClusterRole {
            metadata: ObjectMeta {
                name: Some(if index == 0 {
                    "cluster-admin".to_string()
                } else {
                    format!("cluster-role-{index:03}")
                }),
                ..Default::default()
            },
            rules: Some(rules(index)),
            ..Default::default()
        })
        .collect();
    let role_bindings = (0..500)
        .map(|index| RoleBinding {
            metadata: namespaced_metadata(&format!("role-binding-{index:03}"), index),
            role_ref: RoleRef {
                api_group: "rbac.authorization.k8s.io".to_string(),
                kind: "Role".to_string(),
                name: if index % 17 == 0 {
                    format!("missing-role-{index:03}")
                } else {
                    role_name(index)
                },
            },
            subjects: Some(binding_subjects(index)),
        })
        .collect();
    let cluster_role_bindings = (0..500)
        .map(|index| {
            let mut subjects = binding_subjects(index);
            if index == 0 {
                subjects.push(Subject {
                    kind: "Group".to_string(),
                    name: "system:masters".to_string(),
                    api_group: Some("rbac.authorization.k8s.io".to_string()),
                    ..Default::default()
                });
            }
            ClusterRoleBinding {
                metadata: ObjectMeta {
                    name: Some(format!("cluster-role-binding-{index:03}")),
                    ..Default::default()
                },
                role_ref: RoleRef {
                    api_group: "rbac.authorization.k8s.io".to_string(),
                    kind: "ClusterRole".to_string(),
                    name: if index % 19 == 0 {
                        format!("missing-cluster-role-{index:03}")
                    } else if index == 1 {
                        "cluster-admin".to_string()
                    } else {
                        format!("cluster-role-{index:03}")
                    },
                },
                subjects: Some(subjects),
            }
        })
        .collect();
    RbacInventoryInputs {
        namespaces,
        service_accounts,
        roles,
        cluster_roles,
        role_bindings,
        cluster_role_bindings,
    }
}

pub fn project_inventory(
    inputs: RbacInventoryInputs,
    roles_coverage: RbacCoverageStatus,
    cluster_roles_coverage: RbacCoverageStatus,
) -> RbacInspectionSummary {
    benchmark_rbac_inventory_projection(
        "benchmark",
        inputs.namespaces,
        inputs.service_accounts,
        inputs.roles,
        inputs.cluster_roles,
        inputs.role_bindings,
        inputs.cluster_role_bindings,
        roles_coverage,
        cluster_roles_coverage,
    )
}

pub fn encode<T: serde::Serialize>(value: &T) -> String {
    super::encode_complete_response(value)
}

fn namespaced_metadata(name: &str, index: usize) -> ObjectMeta {
    ObjectMeta {
        name: Some(name.to_string()),
        namespace: Some(format!("tenant-{:03}", index % 250)),
        ..Default::default()
    }
}

fn rules(index: usize) -> Vec<PolicyRule> {
    vec![
        PolicyRule {
            verbs: vec!["get".to_string(), "list".to_string(), "get".to_string()],
            api_groups: Some(vec!["apps".to_string(), String::new(), "apps".to_string()]),
            resources: Some(vec!["deployments".to_string(), "secrets".to_string()]),
            resource_names: Some(vec![
                format!("workload-{index}"),
                format!("workload-{index}"),
            ]),
            non_resource_urls: None,
        },
        PolicyRule {
            verbs: if index.is_multiple_of(11) {
                vec!["*".to_string(), "impersonate".to_string()]
            } else {
                vec!["create".to_string(), "patch".to_string()]
            },
            api_groups: Some(vec!["rbac.authorization.k8s.io".to_string()]),
            resources: Some(if index.is_multiple_of(11) {
                vec!["*".to_string(), "pods/exec".to_string()]
            } else {
                vec!["roles".to_string(), "rolebindings".to_string()]
            }),
            resource_names: None,
            non_resource_urls: None,
        },
    ]
}

fn binding_subjects(index: usize) -> Vec<Subject> {
    let namespace = format!("tenant-{:03}", index % 250);
    let name = service_account_name(index);
    vec![
        Subject {
            kind: "ServiceAccount".to_string(),
            name: name.clone(),
            namespace: Some(namespace.clone()),
            api_group: Some(String::new()),
        },
        Subject {
            kind: "ServiceAccount".to_string(),
            name,
            namespace: Some(namespace),
            api_group: Some(String::new()),
        },
    ]
}

fn service_account_name(index: usize) -> String {
    if matches!(index, 1 | 2) {
        "shared-deployer".to_string()
    } else {
        format!("sa-{index:03}")
    }
}

fn role_name(index: usize) -> String {
    if matches!(index, 1 | 2) {
        "shared-role".to_string()
    } else {
        format!("role-{index:03}")
    }
}
