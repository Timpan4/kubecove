use super::*;

// R11, folded R12/R14/R15: ready cache payload copies and watch-dirty replacement.
mod cache_ready_resource_payload_hit_10000 {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn cache_ready_resource_payload_hit_10000(bencher: codspeed_divan_compat::Bencher) {
        let fixture = resource_cache_fixture();
        fixture.assert_cache_partitioning_and_namespace_normalization();
        let hit = fixture.ready_resource_hit().expect("ready hit");
        assert_eq!(hit.len(), 10_000);
        assert_eq!(
            serde_json::to_value(&hit).expect("serialize ready cache hit"),
            serde_json::to_value(fixture.expected_rows()).expect("serialize expected rows")
        );
        bencher
            .with_inputs(resource_cache_fixture)
            .bench_values(|fixture| {
                black_box(fixture.ready_resource_hit().expect("ready resource hit"))
            });
    }
}

mod cache_all_namespace_to_named_scope_100 {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn cache_all_namespace_to_named_scope_100(bencher: codspeed_divan_compat::Bencher) {
        let fixture = resource_cache_fixture();
        let hit = fixture.named_scope_from_all_hit().expect("named hit");
        let expected = fixture
            .expected_rows()
            .iter()
            .filter(|row| row.namespace.as_deref() == Some("tenant-42"))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(hit.len(), 100);
        assert_eq!(
            serde_json::to_value(&hit).expect("serialize named cache hit"),
            serde_json::to_value(&expected).expect("serialize expected named rows")
        );
        bencher
            .with_inputs(resource_cache_fixture)
            .bench_values(|fixture| {
                black_box(
                    fixture
                        .named_scope_from_all_hit()
                        .expect("named-scope cache hit"),
                )
            });
    }
}

mod cache_topology_payload_hit {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn cache_topology_payload_hit(bencher: codspeed_divan_compat::Bencher) {
        let fixture = resource_cache_fixture();
        let hit = fixture.topology_hit().expect("topology hit");
        assert_eq!(hit.nodes.len(), 2_512);
        assert_eq!(
            serde_json::to_value(&hit).expect("serialize topology cache hit"),
            serde_json::to_value(fixture.expected_topology()).expect("serialize expected topology")
        );
        bencher
            .with_inputs(resource_cache_fixture)
            .bench_values(|fixture| black_box(fixture.topology_hit().expect("topology cache hit")));
    }
}

mod cache_flux_index_payload_hit {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn cache_flux_index_payload_hit(bencher: codspeed_divan_compat::Bencher) {
        assert_eq!(
            resource_cache_fixture().flux_index_hit().expect("Flux hit"),
            2_048
        );
        bencher
            .with_inputs(resource_cache_fixture)
            .bench_values(|fixture| black_box(fixture.flux_index_hit().expect("Flux cache hit")));
    }
}

mod cache_watch_dirty_replacement_10000 {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn cache_watch_dirty_replacement_10000(bencher: codspeed_divan_compat::Bencher) {
        let expected = resource_cache_fixture()
            .dirty_replacement()
            .expect("replacement");
        assert_eq!(expected.len(), 10_000);
        assert_eq!(
            expected[0].status.as_deref(),
            Some("Running after watch replacement")
        );
        bencher
            .with_inputs(resource_cache_fixture)
            .bench_values(|fixture| {
                black_box(fixture.dirty_replacement().expect("dirty replacement"))
            });
    }
}
