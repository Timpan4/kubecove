use super::*;

// R11, folded R12/R14/R15: ready cache payload copies and watch-dirty replacement.
// `bench_refs` keeps fixture teardown (store, seed rows, topology, index) out of timing.
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
            .bench_refs(|fixture| {
                black_box(fixture.ready_resource_hit().expect("ready resource hit"))
            });
    }
}

mod cache_at_capacity {
    use super::*;
    use kubecove_kube::commands::{
        ClusterLiveStore, live_store::BENCHMARK_RESOURCE_CACHE_CAPACITY,
    };
    use tokio::runtime::{Builder, Runtime};

    fn fixture() -> (Runtime, ClusterLiveStore) {
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let store = ClusterLiveStore::default();
        runtime.block_on(async {
            for index in 0..BENCHMARK_RESOURCE_CACHE_CAPACITY {
                store
                    .typed_resources(
                        "fixture".into(),
                        index.to_string(),
                        "Pod".into(),
                        None,
                        || async { Ok(Vec::<ResourceSummary>::new()) },
                    )
                    .await
                    .unwrap();
            }
        });
        (runtime, store)
    }

    #[codspeed_divan_compat::bench]
    fn cache_fill_at_ready_entry_budget(bencher: codspeed_divan_compat::Bencher) {
        bencher.with_inputs(fixture).bench_refs(|(runtime, store)| {
            black_box(
                runtime
                    .block_on(store.typed_resources(
                        "fixture".into(),
                        "next-context".into(),
                        "Pod".into(),
                        None,
                        || async { Ok(Vec::<ResourceSummary>::new()) },
                    ))
                    .unwrap(),
            )
        });
    }
}

mod watch_with_held_snapshot {
    use super::*;
    use kube::api::{DynamicObject, WatchEvent};
    use kubecove_inspection::commands::watched_resource_summary;
    use kubecove_kube::commands::{
        ClusterLiveStore,
        live_store::{ListSnapshot, ListedResources, WatchOrigin},
    };
    use kubecove_lib::models::WatchResourceKind;

    static ROWS: LazyLock<Vec<ResourceSummary>> =
        LazyLock::new(|| resource_cache_fixture().expected_rows().to_vec());

    fn fixture() -> (
        ClusterLiveStore,
        ListSnapshot,
        WatchResourceKind,
        WatchEvent<DynamicObject>,
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let store = ClusterLiveStore::default().applying_watch_rows(watched_resource_summary);
        let load = |store: &ClusterLiveStore| {
            runtime
                .block_on(store.typed_resources(
                    "fixture".into(),
                    "catalog-cluster".into(),
                    "Pod".into(),
                    None,
                    || async {
                        Ok(ListedResources {
                            rows: ROWS.clone(),
                            resource_version: Some("10".into()),
                        })
                    },
                ))
                .unwrap()
        };
        let (head, tail) = (&ROWS[0], &ROWS[1]);
        let object = |row: &ResourceSummary, version: &str| {
            serde_json::from_value(serde_json::json!({
                "apiVersion":"v1", "kind":"Pod",
                "metadata":{"name":row.name, "namespace":row.namespace, "resourceVersion":version},
                "status":{"phase":"Running"}
            }))
            .unwrap()
        };
        let kind = WatchResourceKind {
            kind: "Pod".into(),
            group: Some(String::new()),
            version: Some("v1".into()),
            api_version: Some("v1".into()),
            plural: Some("pods".into()),
            namespaced: Some(true),
        };
        load(&store);
        store.apply_watch_event(
            "fixture",
            "catalog-cluster",
            &kind,
            WatchOrigin {
                namespace: None,
                position: Some("10"),
            },
            &WatchEvent::Modified(object(tail, "11")),
        );
        let held = load(&store);
        assert_eq!(held.resource_version.as_deref(), Some("11"));
        (store, held, kind, WatchEvent::Deleted(object(head, "12")))
    }

    #[codspeed_divan_compat::bench]
    fn cache_watch_delete_with_held_snapshot_10000(bencher: codspeed_divan_compat::Bencher) {
        bencher
            .with_inputs(fixture)
            .bench_refs(|(store, held, kind, event)| {
                store.apply_watch_event(
                    "fixture",
                    "catalog-cluster",
                    kind,
                    WatchOrigin {
                        namespace: None,
                        position: Some("11"),
                    },
                    event,
                );
                black_box(held);
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
            .bench_refs(|fixture| {
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
            .bench_refs(|fixture| black_box(fixture.topology_hit().expect("topology cache hit")));
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
            .bench_refs(|fixture| black_box(fixture.flux_index_hit().expect("Flux cache hit")));
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
            .bench_refs(|fixture| {
                black_box(fixture.dirty_replacement().expect("dirty replacement"))
            });
    }
}
