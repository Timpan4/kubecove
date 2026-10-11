use super::*;

// R05, folded R07: namespace grouping and the production fetch plan.
mod resource_scope_fetch_plan {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn resource_scope_fetch_plan(bencher: codspeed_divan_compat::Bencher) {
        let plan = scope_fetch_plan(SCOPE_INPUT.requests.clone()).expect("fetch plan");
        assert_eq!(plan.len(), 20);
        bencher
            .with_inputs(|| SCOPE_INPUT.requests.clone())
            .bench_values(|requests| black_box(scope_fetch_plan(requests).expect("fetch plan")));
    }
}

mod resource_scope_fetch_plan_all_namespace_promotion {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn resource_scope_fetch_plan_all_namespace_promotion(bencher: codspeed_divan_compat::Bencher) {
        let plan = scope_fetch_plan(ALL_NAMESPACE_SCOPE_REQUESTS.clone()).expect("fetch plan");
        assert_eq!(plan, vec![("typed:Pod".to_string(), None)]);
        bencher
            .with_inputs(|| ALL_NAMESPACE_SCOPE_REQUESTS.clone())
            .bench_values(|requests| black_box(scope_fetch_plan(requests).expect("fetch plan")));
    }
}

// R05, folded R07: prepared batches plus the already-built Flux ownership index.
mod resource_scope_filter_enrich_dedupe {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn resource_scope_filter_enrich_dedupe(bencher: codspeed_divan_compat::Bencher) {
        let rows = assemble_resource_scope(SCOPE_INPUT.clone()).expect("scope fixture");
        assert_eq!(rows.len(), 2_000);
        assert!(rows.iter().any(|row| row.git_ops_owner.is_some()));
        let explicit_owner = rows
            .iter()
            .find(|row| {
                row.kind == "Pod"
                    && row.namespace.as_deref() == Some("tenant-0")
                    && row.name == "ledger-0"
            })
            .expect("explicit metadata owner");
        assert_eq!(
            explicit_owner.git_ops_owner.as_ref().map(|owner| (
                owner.provider.as_str(),
                owner.kind.as_str(),
                owner.name.as_str()
            )),
            Some(("flux", "Kustomization", "ledger-flux"))
        );
        assert_eq!(explicit_owner.argo_app, None);
        assert!(explicit_owner
            .git_ops_owner
            .as_ref()
            .is_some_and(|owner| owner.provenance == "flux-kustomization-labels"));
        assert_eq!(
            explicit_owner.status.as_deref(),
            Some("duplicate-last-row-marker")
        );
        let inventory_owner = rows
            .iter()
            .find(|row| {
                row.kind == "Pod"
                    && row.namespace.as_deref() == Some("tenant-0")
                    && row.name == "ledger-100"
            })
            .expect("ownerless inventory target");
        assert_eq!(
            inventory_owner.git_ops_owner.as_ref().map(|owner| (
                owner.provider.as_str(),
                owner.kind.as_str(),
                owner.name.as_str()
            )),
            Some(("flux", "Kustomization", "owner-Kustomization-0"))
        );
        assert!(inventory_owner
            .git_ops_owner
            .as_ref()
            .is_some_and(|owner| owner.partial));
        bencher
            .with_inputs(|| SCOPE_INPUT.clone())
            .bench_values(|fixture| {
                black_box(assemble_resource_scope(fixture).expect("scope assembly"))
            });
    }
}
