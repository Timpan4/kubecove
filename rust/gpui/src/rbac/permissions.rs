//! Policy-derived grants for one identity, ported from
//! `src/features/rbac/observedPermissions.ts`. This is not an authorization
//! verdict; only the SubjectAccessReview verifier decides access.

use super::{Principal, bindings, binds_principal};
use crate::collate::collate;
use kubecove_lib::models::{
    RbacAccessReviewIdentity, RbacBindingSummary, RbacCoverageStatus, RbacFamily,
    RbacInspectionSummary, RbacRoleSummary,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityKind {
    ServiceAccount,
    User,
    Group,
}

impl IdentityKind {
    pub const ALL: [(Self, &'static str); 3] = [
        (Self::ServiceAccount, "ServiceAccount"),
        (Self::User, "User"),
        (Self::Group, "Group"),
    ];

    pub fn of_subject(kind: &str) -> Self {
        match kind {
            "ServiceAccount" => Self::ServiceAccount,
            "Group" => Self::Group,
            _ => Self::User,
        }
    }
}

/// `InspectorIdentity` in `RbacView.svelte`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub kind: IdentityKind,
    pub name: String,
    pub namespace: Option<String>,
    pub groups: Vec<String>,
}

impl Identity {
    /// `missingServiceAccountNamespace`: `review_identity` substitutes
    /// `default` for a blank namespace, so a review would target another account.
    pub fn missing_namespace(&self) -> bool {
        self.kind == IdentityKind::ServiceAccount
            && self
                .namespace
                .as_deref()
                .is_none_or(|namespace| namespace.trim().is_empty())
    }

    /// `identityLabel`.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            return "No identity selected".into();
        }
        if self.missing_namespace() {
            return format!("ServiceAccount {} (namespace required)", self.name);
        }
        match self.kind {
            IdentityKind::ServiceAccount => format!(
                "system:serviceaccount:{}:{}",
                self.namespace.as_deref().unwrap_or_default(),
                self.name
            ),
            IdentityKind::Group => format!("Group/{}", self.name),
            IdentityKind::User => format!("User/{}", self.name),
        }
    }

    /// The verifier's request identity, with `verify()`'s defaults.
    pub fn review_identity(&self) -> RbacAccessReviewIdentity {
        match self.kind {
            IdentityKind::ServiceAccount => RbacAccessReviewIdentity::ServiceAccount {
                name: self.name.clone(),
                namespace: self
                    .namespace
                    .clone()
                    .filter(|namespace| !namespace.is_empty())
                    .unwrap_or_else(|| "default".into()),
            },
            IdentityKind::Group => RbacAccessReviewIdentity::Group {
                group: self.name.clone(),
            },
            IdentityKind::User => RbacAccessReviewIdentity::User {
                username: self.name.clone(),
                groups: if self.groups.is_empty() {
                    identity_defaults(IdentityKind::User, &self.name)
                } else {
                    self.groups.clone()
                },
            },
        }
    }
}

/// `identityDefaults`.
pub fn identity_defaults(kind: IdentityKind, name: &str) -> Vec<String> {
    match kind {
        IdentityKind::User if name == "system:anonymous" => vec!["system:unauthenticated".into()],
        IdentityKind::User => vec!["system:authenticated".into()],
        _ => vec![],
    }
}

/// `uniqueGroups`: comma-separated, trimmed, first occurrence kept.
pub fn unique_groups(value: &str) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for group in value
        .split(',')
        .map(str::trim)
        .filter(|group| !group.is_empty())
    {
        if !groups.iter().any(|seen| seen == group) {
            groups.push(group.to_owned());
        }
    }
    groups
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub verbs: Vec<String>,
    pub api_groups: Vec<String>,
    pub resources: Vec<String>,
    pub resource_names: Vec<String>,
    pub non_resource_urls: Vec<String>,
    pub cluster: bool,
    pub namespace: Option<String>,
    pub roles: Vec<String>,
    pub bindings: Vec<String>,
}

impl Grant {
    /// `grantSummary`: `verbs · groups/resources`, then `· names: …` when the
    /// rule is limited to named objects.
    pub fn summary(&self) -> String {
        let groups = match self.api_groups.join(", ") {
            joined if joined.is_empty() => "core".to_owned(),
            joined => joined,
        };
        let targets = if self.resources.is_empty() {
            self.non_resource_urls.join(", ")
        } else {
            self.resources.join(", ")
        };
        let mut summary = format!("{} · {groups}/{targets}", self.verbs.join(", "));
        if !self.resource_names.is_empty() {
            summary.push_str(" · names: ");
            summary.push_str(&self.resource_names.join(", "));
        }
        summary
    }

    /// The grant's second line: `scope/namespace · roles`.
    pub fn provenance(&self) -> String {
        let scope = if self.cluster { "cluster" } else { "namespace" };
        let namespace = self
            .namespace
            .as_ref()
            .map_or(String::new(), |namespace| format!("/{namespace}"));
        format!("{scope}{namespace} · {}", self.roles.join(", "))
    }
}

pub struct Observed {
    pub grants: Vec<Grant>,
    pub unknown: bool,
    pub reason: Option<String>,
}

fn role_key(kind: &str, name: &str, namespace: Option<&str>) -> String {
    format!("{kind}/{}/{name}", namespace.unwrap_or("_"))
}

fn binding_key(binding: &RbacBindingSummary) -> String {
    role_key(&binding.kind, &binding.name, binding.namespace.as_deref())
}

const FAMILIES: [(RbacFamily, &str); 4] = [
    (RbacFamily::Roles, "roles"),
    (RbacFamily::ClusterRoles, "clusterRoles"),
    (RbacFamily::RoleBindings, "roleBindings"),
    (RbacFamily::ClusterRoleBindings, "clusterRoleBindings"),
];

/// `observedPermissions`, for the same identity the verifier would review.
pub fn observed(data: &RbacInspectionSummary, identity: &Identity) -> Observed {
    // The binding lookup would read a blank namespace as `default` and show another account's grants.
    if identity.missing_namespace() {
        return Observed {
            grants: vec![],
            unknown: true,
            reason: Some("The ServiceAccount namespace is missing.".into()),
        };
    }
    let principal = Principal::of(&identity.review_identity());
    let incomplete: Vec<&str> = FAMILIES
        .iter()
        .filter(|(family, _)| {
            !data.coverage.iter().any(|entry| {
                entry.family == *family && entry.status == RbacCoverageStatus::Complete
            })
        })
        .map(|(_, label)| *label)
        .collect();
    let roles: HashMap<String, &RbacRoleSummary> = data
        .roles
        .iter()
        .chain(&data.cluster_roles)
        .map(|role| {
            (
                role_key(&role.kind, &role.name, role.namespace.as_deref()),
                role,
            )
        })
        .collect();
    let mut merged: Vec<Grant> = Vec::new();
    let mut missing = false;
    for binding in bindings(data) {
        if !binds_principal(binding, &principal) {
            continue;
        }
        let namespace = (binding.role_ref_kind == "Role")
            .then_some(binding.namespace.as_deref())
            .flatten();
        let Some(role) = roles.get(&role_key(
            &binding.role_ref_kind,
            &binding.role_ref_name,
            namespace,
        )) else {
            missing = true;
            continue;
        };
        let cluster = binding.kind == "ClusterRoleBinding";
        for rule in &role.rules {
            let non_resource_urls = if cluster {
                rule.non_resource_urls.clone()
            } else {
                vec![]
            };
            if rule.resources.is_empty() && non_resource_urls.is_empty() {
                continue;
            }
            let grant = Grant {
                verbs: rule.verbs.clone(),
                api_groups: rule.api_groups.clone(),
                resources: rule.resources.clone(),
                resource_names: rule.resource_names.clone(),
                non_resource_urls,
                cluster,
                namespace: if cluster {
                    None
                } else {
                    binding.namespace.clone()
                },
                roles: vec![role_key(&role.kind, &role.name, role.namespace.as_deref())],
                bindings: vec![binding_key(binding)],
            };
            merge(&mut merged, grant);
        }
    }
    let mut reasons = Vec::new();
    if !incomplete.is_empty() {
        reasons.push(format!(
            "Coverage is partial or unavailable for {}.",
            incomplete.join(", ")
        ));
    }
    if missing {
        reasons.push("A referenced role was not loaded.".into());
    }
    Observed {
        grants: merged,
        unknown: !reasons.is_empty(),
        reason: (!reasons.is_empty()).then(|| reasons.join(" ")),
    }
}

fn merge(merged: &mut Vec<Grant>, grant: Grant) {
    let same = |other: &Grant| {
        other.verbs == grant.verbs
            && other.api_groups == grant.api_groups
            && other.resources == grant.resources
            && other.resource_names == grant.resource_names
            && other.non_resource_urls == grant.non_resource_urls
            && other.cluster == grant.cluster
            && other.namespace == grant.namespace
    };
    match merged.iter_mut().find(|other| same(other)) {
        Some(current) => {
            union_sorted(&mut current.roles, grant.roles);
            union_sorted(&mut current.bindings, grant.bindings);
        }
        None => merged.push(grant),
    }
}

fn union_sorted(into: &mut Vec<String>, more: Vec<String>) {
    into.extend(more);
    into.sort_by(|a, b| collate(a, b, false));
    into.dedup();
}
