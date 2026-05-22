use crate::models::{RbacRiskIndicator, RbacRiskLevel, RbacSubjectSummary};
use std::collections::BTreeSet;

pub(super) fn rule_risks(verbs: &[String], resources: &[String]) -> Vec<RbacRiskIndicator> {
    let verb_set: BTreeSet<&str> = verbs.iter().map(String::as_str).collect();
    let resource_set: BTreeSet<&str> = resources.iter().map(String::as_str).collect();
    let has_wildcard_verb = verb_set.contains("*");
    let has_wildcard_resource = resource_set.contains("*");
    let mut risks = Vec::new();

    if has_wildcard_verb {
        risks.push(risk("high", "Wildcard verbs", "Rule allows every verb."));
    }
    if has_wildcard_resource {
        risks.push(risk(
            "high",
            "Wildcard resources",
            "Rule applies to every resource.",
        ));
    }
    if verb_set
        .iter()
        .any(|verb| matches!(*verb, "impersonate" | "escalate" | "bind"))
    {
        risks.push(risk(
            "high",
            "Privilege escalation verbs",
            "Rule includes impersonate, escalate, or bind.",
        ));
    }
    if (resource_set.contains("secrets") || has_wildcard_resource)
        && (has_wildcard_verb
            || verb_set.iter().any(|verb| {
                matches!(
                    *verb,
                    "get" | "list" | "watch" | "create" | "update" | "patch" | "delete"
                )
            }))
    {
        risks.push(risk(
            "high",
            "Secrets access",
            "Rule can read or change Secret resources.",
        ));
    }
    if (has_wildcard_resource
        || resource_set.iter().any(|resource| {
            matches!(
                *resource,
                "roles" | "rolebindings" | "clusterroles" | "clusterrolebindings"
            )
        }))
        && verb_set
            .iter()
            .any(|verb| matches!(*verb, "create" | "update" | "patch" | "delete"))
    {
        risks.push(risk(
            "medium",
            "RBAC write access",
            "Rule can change RBAC policy resources.",
        ));
    }

    dedupe_risks(risks)
}

pub(super) fn service_account_risks(automount_token: Option<bool>) -> Vec<RbacRiskIndicator> {
    if automount_token == Some(true) {
        vec![risk(
            "medium",
            "Automount token",
            "ServiceAccount explicitly automounts an API token.",
        )]
    } else {
        Vec::new()
    }
}

pub(super) fn binding_risks(
    role_ref_kind: &str,
    role_ref_name: &str,
    subjects: &[RbacSubjectSummary],
) -> Vec<RbacRiskIndicator> {
    let mut risks = Vec::new();
    if role_ref_kind == "ClusterRole" && role_ref_name == "cluster-admin" {
        risks.push(risk(
            "high",
            "Cluster admin binding",
            "Binding references the cluster-admin ClusterRole.",
        ));
    }
    if subjects
        .iter()
        .any(|subject| subject.kind == "Group" && subject.name == "system:masters")
    {
        risks.push(risk(
            "high",
            "System masters subject",
            "Binding includes the system:masters group.",
        ));
    }
    dedupe_risks(risks)
}

pub(super) fn dedupe_subjects(subjects: Vec<RbacSubjectSummary>) -> Vec<RbacSubjectSummary> {
    let mut seen = BTreeSet::new();
    subjects
        .into_iter()
        .filter(|subject| {
            seen.insert(format!(
                "{}:{}:{}",
                subject.kind,
                subject.namespace.as_deref().unwrap_or(""),
                subject.name
            ))
        })
        .collect()
}

pub(super) fn dedupe_risks(risks: Vec<RbacRiskIndicator>) -> Vec<RbacRiskIndicator> {
    let mut seen = BTreeSet::new();
    risks
        .into_iter()
        .filter(|risk| seen.insert((risk.level.clone(), risk.label.clone())))
        .collect()
}

fn risk(level: &str, label: &str, reason: &str) -> RbacRiskIndicator {
    RbacRiskIndicator {
        level: match level {
            "high" => RbacRiskLevel::High,
            "medium" => RbacRiskLevel::Medium,
            _ => RbacRiskLevel::Low,
        },
        label: label.to_string(),
        reason: reason.to_string(),
    }
}
