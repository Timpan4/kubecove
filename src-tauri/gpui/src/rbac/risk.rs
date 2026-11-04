//! Risk inheritance and weighting from `src/features/rbac/risk.ts`.

use super::{bindings, serde_label};
use kubecove_lib::models::{
    RbacBindingSummary, RbacInspectionSummary, RbacRiskIndicator, RbacRiskLevel, RbacRoleSummary,
};
use std::collections::{HashMap, HashSet};

/// Lookup tables for inherited binding and role risks (`buildRbacRiskIndex`).
pub(super) struct RiskIndex<'a> {
    roles: HashMap<(bool, Option<&'a str>, &'a str), &'a RbacRoleSummary>,
    by_account: HashMap<(Option<&'a str>, &'a str), Vec<usize>>,
    by_group: HashMap<&'a str, Vec<usize>>,
    bindings: Vec<&'a RbacBindingSummary>,
}

impl<'a> RiskIndex<'a> {
    pub(super) fn new(data: &'a RbacInspectionSummary) -> Self {
        let mut roles = HashMap::new();
        for role in &data.roles {
            roles
                .entry((true, role.namespace.as_deref(), role.name.as_str()))
                .or_insert(role);
        }
        for role in &data.cluster_roles {
            roles
                .entry((false, None, role.name.as_str()))
                .or_insert(role);
        }
        let bindings: Vec<_> = bindings(data).collect();
        let (mut by_account, mut by_group) = (HashMap::new(), HashMap::new());
        for (position, binding) in bindings.iter().enumerate() {
            for subject in &binding.subjects {
                if subject.kind == "ServiceAccount" {
                    by_account
                        .entry((subject.namespace.as_deref(), subject.name.as_str()))
                        .or_insert_with(Vec::new)
                        .push(position);
                } else if subject.kind == "Group" {
                    by_group
                        .entry(subject.name.as_str())
                        .or_insert_with(Vec::new)
                        .push(position);
                }
            }
        }
        Self {
            roles,
            by_account,
            by_group,
            bindings,
        }
    }

    pub(super) fn role_risks(&self, binding: &RbacBindingSummary) -> Vec<RbacRiskIndicator> {
        let key = if binding.role_ref_kind == "Role" {
            (
                true,
                binding.namespace.as_deref(),
                binding.role_ref_name.as_str(),
            )
        } else {
            (false, None, binding.role_ref_name.as_str())
        };
        match self.roles.get(&key) {
            Some(role) => role.risks.clone(),
            None => vec![RbacRiskIndicator {
                level: RbacRiskLevel::Unknown,
                label: "Missing role reference".into(),
                reason: format!(
                    "{}/{} references {}/{}, which was not loaded.",
                    binding.kind, binding.name, binding.role_ref_kind, binding.role_ref_name
                ),
            }],
        }
    }

    /// Bindings naming the account or its automatic groups, in binding order.
    pub(super) fn account_bindings(
        &self,
        namespace: &str,
        name: &str,
    ) -> Vec<&'a RbacBindingSummary> {
        let mut positions: Vec<usize> = self
            .by_account
            .get(&(Some(namespace), name))
            .cloned()
            .unwrap_or_default();
        for group in automatic_groups(namespace) {
            if let Some(found) = self.by_group.get(group.as_str()) {
                positions.extend(found);
            }
        }
        positions.sort_unstable();
        positions.dedup();
        positions
            .into_iter()
            .map(|position| self.bindings[position])
            .collect()
    }
}

/// `automaticServiceAccountGroups`.
pub fn automatic_groups(namespace: &str) -> [String; 3] {
    [
        "system:serviceaccounts".into(),
        format!("system:serviceaccounts:{namespace}"),
        "system:authenticated".into(),
    ]
}

pub(super) fn unique_risks(risks: Vec<RbacRiskIndicator>) -> Vec<RbacRiskIndicator> {
    let mut seen = HashSet::new();
    risks
        .into_iter()
        .filter(|risk| {
            seen.insert((
                level_weight(&risk.level),
                risk.label.clone(),
                risk.reason.clone(),
            ))
        })
        .collect()
}

fn level_weight(level: &RbacRiskLevel) -> u8 {
    match level {
        RbacRiskLevel::High => 3,
        RbacRiskLevel::Medium => 2,
        RbacRiskLevel::Low => 1,
        RbacRiskLevel::Unknown => 0,
    }
}

pub(super) fn risk_weight(risks: &[RbacRiskIndicator]) -> u8 {
    if risks.iter().any(|risk| risk.level == RbacRiskLevel::High) {
        3
    } else if risks.iter().any(|risk| risk.level == RbacRiskLevel::Medium) {
        2
    } else {
        u8::from(!risks.is_empty())
    }
}

/// `riskSummaryLabel`.
pub fn risk_summary_label(risks: &[RbacRiskIndicator]) -> String {
    let Some(highest) = risks
        .iter()
        .rev()
        .max_by_key(|risk| level_weight(&risk.level))
    else {
        return "No flags".into();
    };
    let suffix = if risks.len() == 1 { "flag" } else { "flags" };
    format!(
        "{}: {} {suffix}",
        serde_label(&highest.level).to_uppercase(),
        risks.len()
    )
}
