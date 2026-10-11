use super::*;

// R01, folded R02/R03, and the Namespace age/name projection.
mod typed_resource_summaries_10000 {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn typed_resource_summaries_10000(bencher: codspeed_divan_compat::Bencher) {
        assert_eq!(TYPED_OUTPUT.len(), 10_015);
        assert!(TYPED_OUTPUT
            .iter()
            .any(|row| row.kind == "Job" && row.status.as_deref() == Some("Complete")));
        assert!(TYPED_OUTPUT
            .iter()
            .any(|row| row.kind == "CustomResourceDefinition" && row.created_at.is_none()));
        let sidecar_pod = TYPED_OUTPUT
            .iter()
            .find(|row| row.name == "ledger-2")
            .expect("multi-container Pod summary");
        assert_eq!(sidecar_pod.status.as_deref(), Some("Running"));
        assert_eq!(sidecar_pod.ready.as_deref(), Some("True"));
        assert_eq!(sidecar_pod.restarts, Some(4));
        assert_eq!(
            sidecar_pod.health_assessment.state,
            kubecove_lib::models::HealthAssessmentState::Healthy
        );
        let duplicate_ready_fixture = SUMMARY_INPUT
            .pods
            .iter()
            .find(|pod| pod.metadata.name.as_deref() == Some("ledger-2"))
            .expect("Pod fixture with multiple Ready conditions");
        assert_eq!(
            duplicate_ready_fixture
                .status
                .as_ref()
                .and_then(|status| status.conditions.as_ref())
                .expect("Pod conditions")
                .iter()
                .filter(|condition| condition.type_ == "Ready")
                .count(),
            2
        );
        let no_ready_condition = TYPED_OUTPUT
            .iter()
            .find(|row| row.name == "ledger-3")
            .expect("Pod with non-Ready conditions");
        assert_eq!(no_ready_condition.ready, None);
        assert_eq!(
            no_ready_condition.health_assessment.state,
            kubecove_lib::models::HealthAssessmentState::Healthy
        );
        let false_ready_condition = TYPED_OUTPUT
            .iter()
            .find(|row| row.name == "ledger-9")
            .expect("Pod with false Ready condition");
        assert_eq!(false_ready_condition.ready.as_deref(), Some("False"));
        assert_eq!(
            false_ready_condition.health_assessment.state,
            kubecove_lib::models::HealthAssessmentState::Degraded
        );
        bencher
            .with_inputs(|| SUMMARY_INPUT.clone())
            .bench_values(|fixture| black_box(typed_summaries_at(&fixture, fixed_now())));
    }
}

// R17 Namespace projection folded into the typed summary workload.
mod namespace_name_age_projection_100 {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn namespace_name_age_projection_100(bencher: codspeed_divan_compat::Bencher) {
        let expected = namespace_summaries_at(&NAMESPACES, fixed_now());
        assert_eq!(expected.len(), 100);
        assert_eq!(expected[0].name, "tenant-0");
        assert_eq!(expected[99].age, "unknown");
        assert!(expected[99].created_at.is_none());
        bencher
            .with_inputs(|| NAMESPACES.clone())
            .bench_values(|namespaces| black_box(namespace_summaries_at(&namespaces, fixed_now())));
    }
}

// R04, folded dynamic-health and ownership projection.
mod dynamic_resource_summaries_10000 {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn dynamic_resource_summaries_10000(bencher: codspeed_divan_compat::Bencher) {
        assert_eq!(DYNAMIC_OUTPUT.len(), 10_000);
        assert!(DYNAMIC_OUTPUT.iter().any(|row| row.health_assessment.state
            == kubecove_lib::models::HealthAssessmentState::Degraded));
        let phase_precedence = DYNAMIC_OUTPUT
            .iter()
            .find(|row| row.name == "ledger-0")
            .expect("dynamic resource with phase and Ready evidence");
        assert_eq!(phase_precedence.status.as_deref(), Some("Reconciled"));
        assert_eq!(
            phase_precedence.health_assessment.state,
            kubecove_lib::models::HealthAssessmentState::Degraded
        );
        let malformed_ready = DYNAMIC_OUTPUT
            .iter()
            .find(|row| row.name == "ledger-1")
            .expect("dynamic resource with malformed and multiple conditions");
        assert_eq!(malformed_ready.status, None);
        assert_eq!(
            malformed_ready.health_assessment.state,
            kubecove_lib::models::HealthAssessmentState::Degraded
        );
        let duplicate_ready = DYNAMIC_OUTPUT
            .iter()
            .find(|row| row.name == "ledger-3")
            .expect("dynamic resource with conflicting Ready conditions");
        assert_eq!(duplicate_ready.status.as_deref(), Some("Ready: True"));
        assert_eq!(
            duplicate_ready.health_assessment.state,
            kubecove_lib::models::HealthAssessmentState::Degraded
        );
        let unrecognized_conditions = DYNAMIC_OUTPUT
            .iter()
            .find(|row| row.name == "ledger-4")
            .expect("dynamic resource with malformed conditions only");
        assert_eq!(unrecognized_conditions.status, None);
        assert_eq!(
            unrecognized_conditions.health_assessment.state,
            kubecove_lib::models::HealthAssessmentState::NotEvaluated
        );
        bencher
            .with_inputs(|| DYNAMIC_INPUT.clone())
            .bench_values(|fixture| black_box(dynamic_summaries_at(&fixture, fixed_now())));
    }
}

mod dynamic_resource_identity_health_edges {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn dynamic_resource_identity_health_edges(bencher: codspeed_divan_compat::Bencher) {
        assert_eq!(DYNAMIC_EDGE_OUTPUT.len(), 2);
        assert!(DYNAMIC_EDGE_OUTPUT
            .iter()
            .any(|row| row.argo_app.as_deref() == Some("checkout-app")));
        assert!(DYNAMIC_EDGE_OUTPUT
            .iter()
            .any(|row| row.namespace.is_none() && row.namespaced == Some(false)));
        let application = DYNAMIC_EDGE_OUTPUT
            .iter()
            .find(|row| row.name == "checkout-app")
            .expect("Argo CD Application summary");
        assert_eq!(application.status, None);
        assert_eq!(
            application.health_assessment.state,
            kubecove_lib::models::HealthAssessmentState::Degraded
        );
        assert_eq!(
            application.health,
            kubecove_lib::models::ResourceHealth::Degraded
        );
        let widget = DYNAMIC_EDGE_OUTPUT
            .iter()
            .find(|row| row.name == "cluster-widget")
            .expect("cluster-scoped Widget summary");
        assert_eq!(widget.status.as_deref(), Some("Unknown"));
        assert_eq!(
            widget.health_assessment.state,
            kubecove_lib::models::HealthAssessmentState::Healthy
        );
        assert_eq!(widget.health, kubecove_lib::models::ResourceHealth::Healthy);
        bencher
            .with_inputs(|| DYNAMIC_EDGE_INPUT.clone())
            .bench_values(|fixtures| black_box(dynamic_edge_summaries_at(&fixtures, fixed_now())));
    }
}

// R17: CRD served-version projection, ordering, and duplicate removal.
mod crd_catalog_projection_1000 {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn crd_catalog_projection_1000(bencher: codspeed_divan_compat::Bencher) {
        let expected = crd_resource_catalog(CRDS.clone());
        assert_eq!(expected.len(), 1_000);
        assert!(expected.iter().all(|kind| kind.version == "v1"));
        assert!(expected.iter().any(|kind| !kind.namespaced));
        assert!(expected.windows(2).all(|rows| {
            (
                &rows[0].group,
                &rows[0].kind,
                &rows[0].version,
                &rows[0].plural,
            ) <= (
                &rows[1].group,
                &rows[1].kind,
                &rows[1].version,
                &rows[1].plural,
            )
        }));
        bencher
            .with_inputs(|| CRDS.clone())
            .bench_values(|definitions| black_box(crd_resource_catalog(definitions)));
    }
}

// R18: parser, merge, collision handling, and context summary projection.
mod kubeconfig_parse_merge_context_projection {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn kubeconfig_parse_merge_context_projection(bencher: codspeed_divan_compat::Bencher) {
        let contexts = kubeconfig_context_projection(&KUBECONFIG_INPUT)
            .expect("credential-free source fixtures parse and merge");
        assert!(!contexts.is_empty());
        bencher
            .with_inputs(|| KUBECONFIG_INPUT.clone())
            .bench_values(|fixture| {
                black_box(kubeconfig_context_projection(&fixture).expect("kubeconfig fixture"))
            });
    }
}

// R06: fetched Kustomization and HelmRelease owner objects through the production bounds.
mod flux_owner_index_64_objects_2048_entries {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn flux_owner_index_64_objects_2048_entries(bencher: codspeed_divan_compat::Bencher) {
        let expected = build_flux_index_fixture(FLUX_INPUT.clone());
        assert_eq!(expected.retained_items, 2_048);
        assert!(expected.partial);
        let first_owner = Some((
            "flux".to_string(),
            "Kustomization".to_string(),
            "owner-Kustomization-0".to_string(),
        ));
        let [duplicate_owner, grouped_owner, version_alias_owner, empty_name_owner, missing_version_owner] =
            FLUX_INPUT.expected_owner_lookups();
        assert_eq!(duplicate_owner, first_owner);
        assert_eq!(grouped_owner, first_owner);
        assert_eq!(version_alias_owner, first_owner);
        assert_eq!(empty_name_owner, None);
        assert_eq!(missing_version_owner, None);
        bencher
            .with_inputs(|| FLUX_INPUT.clone())
            .bench_values(|fixture| black_box(build_flux_index_fixture(fixture)));
    }
}
