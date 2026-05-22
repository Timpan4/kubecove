use super::*;

fn values(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

#[test]
fn flags_wildcard_and_secrets_rules() {
    let risks = rule_risks(&values(&["*"]), &values(&["secrets", "pods"]));

    assert!(risks.iter().any(|risk| risk.label == "Wildcard verbs"));
    assert!(risks.iter().any(|risk| risk.label == "Secrets access"));
}

#[test]
fn flags_privilege_escalation_verbs() {
    let risks = rule_risks(&values(&["bind"]), &values(&["clusterroles"]));

    assert!(risks
        .iter()
        .any(|risk| risk.label == "Privilege escalation verbs"));
}

#[test]
fn summarizes_namespace_subjects_without_duplicates() {
    let binding = RbacBindingSummary {
        cluster: "kind-dev".to_string(),
        kind: "RoleBinding".to_string(),
        name: "readers".to_string(),
        namespace: Some("payments".to_string()),
        age: "1h".to_string(),
        created_at: None,
        role_ref_kind: "Role".to_string(),
        role_ref_name: "reader".to_string(),
        subjects: vec![
            RbacSubjectSummary {
                kind: "User".to_string(),
                name: "alice".to_string(),
                namespace: None,
            },
            RbacSubjectSummary {
                kind: "User".to_string(),
                name: "alice".to_string(),
                namespace: None,
            },
        ],
        risks: Vec::new(),
    };

    let summaries = namespace_access_summary("kind-dev", &[], &[], &[binding], &[]);

    assert_eq!(summaries[0].namespace, "payments");
    assert_eq!(summaries[0].role_bindings, 1);
    assert_eq!(summaries[0].bound_subjects.len(), 1);
}

#[test]
fn role_binding_service_account_subject_inherits_binding_namespace() {
    let subject = Subject {
        api_group: None,
        kind: "ServiceAccount".to_string(),
        name: "api".to_string(),
        namespace: None,
    };

    let summary = subject_summary(subject, Some("payments"));

    assert_eq!(summary.namespace, Some("payments".to_string()));
}

#[test]
fn cluster_role_binding_service_accounts_roll_up_to_namespace_access() {
    let binding = RbacBindingSummary {
        cluster: "kind-dev".to_string(),
        kind: "ClusterRoleBinding".to_string(),
        name: "api-admin".to_string(),
        namespace: None,
        age: "1h".to_string(),
        created_at: None,
        role_ref_kind: "ClusterRole".to_string(),
        role_ref_name: "cluster-admin".to_string(),
        subjects: vec![RbacSubjectSummary {
            kind: "ServiceAccount".to_string(),
            name: "api".to_string(),
            namespace: Some("payments".to_string()),
        }],
        risks: binding_risks("ClusterRole", "cluster-admin", &[]),
    };

    let summaries = namespace_access_summary("kind-dev", &[], &[], &[], &[binding]);

    assert_eq!(summaries[0].namespace, "payments");
    assert_eq!(summaries[0].role_bindings, 1);
    assert_eq!(summaries[0].bound_subjects[0].name, "api");
    assert_eq!(summaries[0].risks[0].label, "Cluster admin binding");
}
