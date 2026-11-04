//! RBAC Cockpit logic ported from `src/features/rbac/cockpitModel.ts`,
//! `risk.ts` and `queueModel.ts`. Items are built once per inspection and
//! category; rendering reads them without allocating.

pub mod permissions;
pub mod review;
mod risk;

pub use risk::{automatic_groups, risk_summary_label};

use crate::collate::collate;
use kubecove_lib::models::{
    RbacBindingSummary, RbacInspectionSummary, RbacRiskIndicator, RbacRiskLevel, RbacRoleSummary,
    RbacRuleSummary, RbacSubjectSummary,
};
use risk::{RiskIndex, risk_weight, unique_risks};

/// `RbacView` in `surfaceModel.ts`, in the cockpit's category order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RbacView {
    NamespaceAccess,
    Roles,
    ClusterRoles,
    Bindings,
    ServiceAccounts,
}

impl RbacView {
    pub const ALL: [Self; 5] = [
        Self::NamespaceAccess,
        Self::Roles,
        Self::ClusterRoles,
        Self::Bindings,
        Self::ServiceAccounts,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::NamespaceAccess => "Namespace Access",
            Self::Roles => "Roles",
            Self::ClusterRoles => "Cluster Roles",
            Self::Bindings => "Bindings",
            Self::ServiceAccounts => "Service Accounts",
        }
    }

    /// `REVIEWABLE_RBAC_CATEGORIES`: every category except Namespace Access.
    pub fn reviewable(self) -> bool {
        self != Self::NamespaceAccess
    }

    pub fn count(self, data: &RbacInspectionSummary) -> usize {
        match self {
            Self::NamespaceAccess => data.namespace_access.len(),
            Self::Roles => data.roles.len(),
            Self::ClusterRoles => data.cluster_roles.len(),
            Self::Bindings => data.role_bindings.len() + data.cluster_role_bindings.len(),
            Self::ServiceAccounts => data.service_accounts.len(),
        }
    }
}

/// `RbacRiskBucket` with its filter label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bucket {
    All,
    High,
    Medium,
    Low,
    None,
    Unknown,
}

impl Bucket {
    pub const ALL: [(Self, &'static str); 6] = [
        (Self::All, "All"),
        (Self::High, "High"),
        (Self::Medium, "Medium"),
        (Self::Low, "Low"),
        (Self::None, "No flags"),
        (Self::Unknown, "Unknown"),
    ];

    fn matches(self, risks: &[RbacRiskIndicator]) -> bool {
        let level = match self {
            Self::All => return true,
            Self::None => return risks.is_empty(),
            Self::High => RbacRiskLevel::High,
            Self::Medium => RbacRiskLevel::Medium,
            Self::Low => RbacRiskLevel::Low,
            Self::Unknown => RbacRiskLevel::Unknown,
        };
        risks.iter().any(|risk| risk.level == level)
    }
}

/// The object an item stands for, as an index into its inspection list.
/// Bindings index `role_bindings` followed by `cluster_role_bindings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object {
    Account(usize),
    Role(usize),
    ClusterRole(usize),
    Binding(usize),
    Namespace(usize),
}

pub enum ObjectRef<'a> {
    Account(&'a kubecove_lib::models::ServiceAccountSummary),
    Role(&'a RbacRoleSummary),
    Binding(&'a RbacBindingSummary),
    Namespace(&'a kubecove_lib::models::RbacNamespaceAccessSummary),
}

impl Object {
    pub fn get(self, data: &RbacInspectionSummary) -> ObjectRef<'_> {
        match self {
            Self::Account(index) => ObjectRef::Account(&data.service_accounts[index]),
            Self::Role(index) => ObjectRef::Role(&data.roles[index]),
            Self::ClusterRole(index) => ObjectRef::Role(&data.cluster_roles[index]),
            Self::Binding(index) => ObjectRef::Binding(binding(data, index)),
            Self::Namespace(index) => ObjectRef::Namespace(&data.namespace_access[index]),
        }
    }
}

fn binding(data: &RbacInspectionSummary, index: usize) -> &RbacBindingSummary {
    data.role_bindings
        .get(index)
        .unwrap_or_else(|| &data.cluster_role_bindings[index - data.role_bindings.len()])
}

/// All bindings in `[...roleBindings, ...clusterRoleBindings]` order.
pub fn bindings(data: &RbacInspectionSummary) -> impl Iterator<Item = &RbacBindingSummary> {
    data.role_bindings.iter().chain(&data.cluster_role_bindings)
}

/// `RbacCockpitItem` with its search text precomputed.
pub struct CockpitItem {
    pub category: RbacView,
    pub key: String,
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
    pub risks: Vec<RbacRiskIndicator>,
    pub object: Object,
    search: String,
}

/// `objectKey`: `${category}:${kind}:${namespace ?? "_"}:${name}`.
pub fn object_key(category: RbacView, kind: &str, namespace: Option<&str>, name: &str) -> String {
    format!(
        "{}:{kind}:{}:{name}",
        category.label(),
        namespace.unwrap_or("_")
    )
}

/// `cockpitItems`: the category's objects with resolved risks, sorted by
/// highest risk, then findings, then `namespace/name`.
pub fn cockpit_items(data: &RbacInspectionSummary, category: RbacView) -> Vec<CockpitItem> {
    let index = matches!(category, RbacView::Bindings | RbacView::ServiceAccounts)
        .then(|| RiskIndex::new(data));
    let objects: Vec<Object> = match category {
        RbacView::Roles => (0..data.roles.len()).map(Object::Role).collect(),
        RbacView::ClusterRoles => (0..data.cluster_roles.len())
            .map(Object::ClusterRole)
            .collect(),
        RbacView::Bindings => (0..data.role_bindings.len() + data.cluster_role_bindings.len())
            .map(Object::Binding)
            .collect(),
        RbacView::ServiceAccounts => (0..data.service_accounts.len())
            .map(Object::Account)
            .collect(),
        RbacView::NamespaceAccess => (0..data.namespace_access.len())
            .map(Object::Namespace)
            .collect(),
    };
    let mut items: Vec<CockpitItem> = objects
        .into_iter()
        .map(|object| item(data, index.as_ref(), category, object))
        .collect();
    items.sort_by(|a, b| {
        risk_weight(&b.risks)
            .cmp(&risk_weight(&a.risks))
            .then(b.risks.len().cmp(&a.risks.len()))
            .then_with(|| {
                collate(
                    &format!("{}/{}", a.namespace.as_deref().unwrap_or(""), a.name),
                    &format!("{}/{}", b.namespace.as_deref().unwrap_or(""), b.name),
                    false,
                )
            })
    });
    items
}

fn item(
    data: &RbacInspectionSummary,
    index: Option<&RiskIndex>,
    category: RbacView,
    object: Object,
) -> CockpitItem {
    let mut search = Vec::<&str>::new();
    let (kind, name, namespace, own_risks) = match object.get(data) {
        ObjectRef::Account(account) => (
            "ServiceAccount",
            &account.name,
            Some(&account.namespace),
            &account.risks,
        ),
        ObjectRef::Role(role) => {
            for rule in &role.rules {
                search.extend(rule_words(rule));
            }
            (
                role.kind.as_str(),
                &role.name,
                role.namespace.as_ref(),
                &role.risks,
            )
        }
        ObjectRef::Binding(binding) => {
            search.push(&binding.role_ref_name);
            search.push(&binding.role_ref_kind);
            for subject in &binding.subjects {
                search.extend([
                    subject.kind.as_str(),
                    &subject.name,
                    subject.namespace.as_deref().unwrap_or(""),
                ]);
            }
            (
                binding.kind.as_str(),
                &binding.name,
                binding.namespace.as_ref(),
                &binding.risks,
            )
        }
        ObjectRef::Namespace(access) => (
            "Namespace",
            &access.namespace,
            Some(&access.namespace),
            &access.risks,
        ),
    };
    let risks = match (object, index) {
        (Object::Binding(position), Some(index)) => {
            let binding = binding(data, position);
            let mut risks = binding.risks.clone();
            risks.extend(index.role_risks(binding));
            unique_risks(risks)
        }
        (Object::Account(position), Some(index)) => {
            let account = &data.service_accounts[position];
            let mut risks = account.risks.clone();
            for binding in index.account_bindings(&account.namespace, &account.name) {
                risks.extend(binding.risks.iter().cloned());
                risks.extend(index.role_risks(binding));
            }
            let incomplete: Vec<String> = data
                .coverage
                .iter()
                .filter(|entry| {
                    entry.status != kubecove_lib::models::RbacCoverageStatus::Complete
                        && entry.family != kubecove_lib::models::RbacFamily::ServiceAccounts
                })
                .map(|entry| {
                    format!(
                        "{} {}",
                        serde_label(&entry.family),
                        serde_label(&entry.status)
                    )
                })
                .collect();
            if !incomplete.is_empty() {
                risks.push(RbacRiskIndicator {
                    level: RbacRiskLevel::Unknown,
                    label: "Incomplete RBAC coverage".into(),
                    reason: format!("Coverage is {}.", incomplete.join(", ")),
                });
            }
            unique_risks(risks)
        }
        _ => own_risks.clone(),
    };
    let mut text = vec![name.as_str(), namespace.map_or("", String::as_str), kind];
    text.extend(search);
    let search = text
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    CockpitItem {
        category,
        key: object_key(category, kind, namespace.map(String::as_str), name),
        name: name.clone(),
        namespace: namespace.cloned(),
        kind: kind.to_owned(),
        risks,
        object,
        search,
    }
}

fn rule_words(rule: &RbacRuleSummary) -> impl Iterator<Item = &str> {
    rule.verbs
        .iter()
        .chain(&rule.api_groups)
        .chain(&rule.resources)
        .chain(&rule.non_resource_urls)
        .map(String::as_str)
}

/// The wire spelling of a serde enum, such as `roleBindings` or `partial`.
pub fn serde_label<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// `filterCockpitItems`, returning indices into `items`.
pub fn filter_items(items: &[CockpitItem], bucket: Bucket, search: &str) -> Vec<usize> {
    let needle = search.trim().to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| bucket.matches(&item.risks))
        .filter(|(_, item)| needle.is_empty() || item.search.contains(&needle))
        .map(|(index, _)| index)
        .collect()
}

/// Each risk filter's item count, in [`Bucket::ALL`] order and before search.
pub fn bucket_counts(items: &[CockpitItem]) -> [usize; 6] {
    Bucket::ALL.map(|(bucket, _)| {
        items
            .iter()
            .filter(|item| bucket.matches(&item.risks))
            .count()
    })
}

/// `ruleText`.
pub fn rule_text(rule: &RbacRuleSummary) -> String {
    let group = match rule.api_groups.join(",") {
        joined if joined.is_empty() => "core".to_owned(),
        joined => joined,
    };
    let resources = rule
        .resources
        .iter()
        .map(|resource| format!("{group}/{resource}"))
        .collect::<Vec<_>>()
        .join(", ");
    [
        rule.verbs.join(", "),
        resources,
        rule.non_resource_urls.join(", "),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

/// `subjectLabel` from `risk.ts`: `Kind:namespace/name` or `Kind:name`.
pub fn subject_label(subject: &RbacSubjectSummary) -> String {
    match &subject.namespace {
        Some(namespace) => format!("{}:{namespace}/{}", subject.kind, subject.name),
        None => format!("{}:{}", subject.kind, subject.name),
    }
}

/// `subjectListLabel`.
pub fn subject_list_label(subjects: &[RbacSubjectSummary], limit: usize) -> String {
    if subjects.is_empty() {
        return "-".into();
    }
    let visible = subjects
        .iter()
        .take(limit)
        .map(subject_label)
        .collect::<Vec<_>>()
        .join(", ");
    if subjects.len() <= limit {
        visible
    } else {
        format!("{visible}, +{} more", subjects.len() - limit)
    }
}

/// `queueTags`.
pub fn queue_tags(data: &RbacInspectionSummary, item: &CockpitItem) -> Vec<String> {
    match item.object.get(data) {
        ObjectRef::Account(account) => vec![
            format!("{} bindings", binding_sources(data, item).len()),
            format!("{} secrets", account.secrets_count),
            if account.automount_token == Some(false) {
                "token off".into()
            } else {
                "token posture".into()
            },
        ],
        ObjectRef::Role(role) => vec![
            format!("{} rules", role.rules_count),
            role.namespace
                .clone()
                .unwrap_or_else(|| "cluster-wide".into()),
        ],
        ObjectRef::Binding(binding) => vec![
            format!("{} subjects", binding.subjects.len()),
            format!("{}/{}", binding.role_ref_kind, binding.role_ref_name),
        ],
        ObjectRef::Namespace(access) => vec![
            format!("{} identities", access.service_accounts),
            format!("{} bindings", access.role_bindings),
        ],
    }
}

/// `queueSummary`.
pub fn queue_summary(data: &RbacInspectionSummary, item: &CockpitItem) -> String {
    if !item.risks.is_empty() {
        return item
            .risks
            .iter()
            .take(2)
            .map(|risk| risk.label.as_str())
            .collect::<Vec<_>>()
            .join(" · ");
    }
    if let ObjectRef::Binding(binding) = item.object.get(data) {
        return subject_list_label(&binding.subjects, 2);
    }
    "No policy heuristic flags for this object.".into()
}

/// Whether a binding names the account directly or through its automatic groups.
pub fn binds_account(binding: &RbacBindingSummary, namespace: &str, name: &str) -> bool {
    let groups = automatic_groups(namespace);
    binding.subjects.iter().any(|subject| {
        (subject.kind == "ServiceAccount"
            && subject.name == name
            && subject.namespace.as_deref() == Some(namespace))
            || (subject.kind == "Group" && groups.contains(&subject.name))
    })
}

/// Whether a binding references the role, matching the namespace for Roles.
pub fn binds_role(binding: &RbacBindingSummary, role: &RbacRoleSummary) -> bool {
    binding.role_ref_kind == role.kind
        && binding.role_ref_name == role.name
        && (role.kind != "Role" || binding.namespace == role.namespace)
}

/// `bindingSources`: the bindings that explain the selected object.
pub fn binding_sources<'a>(
    data: &'a RbacInspectionSummary,
    item: &CockpitItem,
) -> Vec<&'a RbacBindingSummary> {
    match item.object.get(data) {
        ObjectRef::Account(account) => bindings(data)
            .filter(|binding| binds_account(binding, &account.namespace, &account.name))
            .collect(),
        ObjectRef::Role(role) => bindings(data)
            .filter(|binding| binds_role(binding, role))
            .collect(),
        ObjectRef::Binding(binding) => vec![binding],
        ObjectRef::Namespace(_) => bindings(data)
            .filter(|binding| binding.namespace == item.namespace)
            .collect(),
    }
}
