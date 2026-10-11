use codspeed_divan_compat::{black_box, Bencher};
use kubecove_lib::commands::bench_support::domain_catalog::domain_rbac;
use kubecove_lib::models::{RbacCoverageStatus, RbacInspectionSummary};

fn complete_inventory() -> RbacInspectionSummary {
    domain_rbac::project_inventory(
        domain_rbac::inventory_inputs(),
        RbacCoverageStatus::Complete,
        RbacCoverageStatus::Complete,
    )
}

mod rbac_inventory_projection_complete_coverage {
    use super::*;

    // R36 and folded R37: raw ServiceAccount/Role/ClusterRole/binding
    // projection, rule normalization, inherited risks, and namespace rollup.
    #[codspeed_divan_compat::bench]
    fn rbac_inventory_projection_complete_coverage(bencher: Bencher) {
        let inputs = domain_rbac::inventory_inputs();
        let expected = domain_rbac::project_inventory(
            inputs.clone(),
            RbacCoverageStatus::Complete,
            RbacCoverageStatus::Complete,
        );
        assert_eq!(expected.service_accounts.len(), 500);
        assert_eq!(expected.roles.len(), 500);
        assert_eq!(expected.cluster_roles.len(), 500);
        assert_eq!(expected.role_bindings.len(), 500);
        assert_eq!(expected.cluster_role_bindings.len(), 500);
        assert_eq!(expected.namespace_access.len(), 250);
        assert_eq!(expected.namespace_access[0].namespace, "tenant-000");
        assert_eq!(expected.namespace_access[249].namespace, "tenant-249");
        assert!(expected.namespace_access.iter().all(|namespace| {
            namespace
                .bound_subjects
                .iter()
                .any(|subject| subject.name == "system:masters")
        }));
        assert_eq!(expected.role_bindings[0].subjects.len(), 2);
        assert!(expected.role_bindings[0].subjects.iter().all(|subject| {
            subject.kind == "ServiceAccount"
                && subject.name == "sa-000"
                && subject.namespace.as_deref() == Some("tenant-000")
        }));
        assert!(expected.role_bindings[0]
            .risks
            .iter()
            .any(|risk| risk.label == "Dangling role reference"));
        assert_eq!(
            expected.roles[0].rules[0].verbs,
            vec!["get".to_string(), "list".to_string()]
        );
        for risk_label in [
            "Secrets access",
            "Wildcard verbs",
            "Wildcard resources",
            "Privilege escalation verbs",
            "RBAC write access",
            "Workload interactive access",
        ] {
            assert!(expected.roles[0]
                .risks
                .iter()
                .any(|risk| risk.label == risk_label));
        }
        assert!(expected.service_accounts[0]
            .risks
            .iter()
            .any(|risk| risk.label == "Automount token"));
        assert!(expected.cluster_role_bindings[0]
            .subjects
            .iter()
            .any(|subject| subject.name == "system:masters"));
        assert!(expected.cluster_role_bindings[1]
            .risks
            .iter()
            .any(|risk| risk.label == "Cluster admin binding"));
        assert_eq!(
            expected
                .roles
                .iter()
                .filter(|role| role.name == "shared-role")
                .map(|role| role.namespace.as_deref())
                .collect::<Vec<_>>(),
            [Some("tenant-001"), Some("tenant-002")]
        );
        for binding in &expected.role_bindings[1..=2] {
            assert!(!binding
                .risks
                .iter()
                .any(|risk| risk.label == "Dangling role reference"));
        }

        bencher
            .with_inputs(|| inputs.clone())
            .bench_values(|inputs| {
                black_box(domain_rbac::project_inventory(
                    inputs,
                    RbacCoverageStatus::Complete,
                    RbacCoverageStatus::Complete,
                ))
            });
    }
}

mod rbac_inventory_projection_partial_coverage {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn rbac_inventory_projection_partial_coverage(bencher: Bencher) {
        let inputs = domain_rbac::inventory_inputs();
        let expected = domain_rbac::project_inventory(
            inputs.clone(),
            RbacCoverageStatus::Partial,
            RbacCoverageStatus::Unavailable,
        );
        assert!(expected.role_bindings[0]
            .risks
            .iter()
            .any(|risk| risk.label == "Referenced role unavailable"));
        assert!(expected.cluster_role_bindings[0]
            .risks
            .iter()
            .any(|risk| risk.label == "Referenced role unavailable"));
        assert_eq!(expected.namespace_access.len(), 250);

        bencher
            .with_inputs(|| inputs.clone())
            .bench_values(|inputs| {
                black_box(domain_rbac::project_inventory(
                    inputs,
                    RbacCoverageStatus::Partial,
                    RbacCoverageStatus::Unavailable,
                ))
            });
    }
}

mod rbac_inventory_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn rbac_inventory_json_encoding(bencher: Bencher) {
        let output = complete_inventory();
        let json = domain_rbac::encode(&output);
        assert!(json.contains("namespaceAccess"));
        assert!(json.contains("cluster-role-binding-499"));
        assert!(json.contains("unknown"));
        bencher
            .with_inputs(|| output.clone())
            .bench_values(|summary| black_box(domain_rbac::encode(&summary)));
    }
}
