use super::*;

#[test]
fn trace_clear_resets_counts_and_refresh_counts_evicted_entries() {
    tauri::async_runtime::block_on(async {
        let store = ClusterLiveStore::default();
        store
            .namespaces(
                "private-source".into(),
                "private-context".into(),
                || async {
                    Ok::<_, AppError>(vec![NamespaceSummary {
                        name: "private-namespace".into(),
                        age: "1m".into(),
                        created_at: None,
                    }])
                },
            )
            .await
            .expect("namespace fixture");
        store.clear_diagnostic_counters();
        let snapshot = store.namespaces.diagnostics();
        assert_eq!(snapshot.misses, 0);
        assert_eq!(snapshot.hits, 0);
        assert_eq!(snapshot.ready, 1);
        assert_eq!(snapshot.retained_items, 1);
        assert_eq!(
            store.refresh_view("private-source", "private-context", &[], &[]),
            1
        );
        let snapshot = store.namespaces.diagnostics();
        assert_eq!(snapshot.evictions, 1);
        assert_eq!(snapshot.ready, 0);
        assert_eq!(snapshot.retained_items, 0);
    });
}

#[test]
fn cache_observations_count_reads_and_restore_dirty_payload_without_identifiers() {
    tauri::async_runtime::block_on(async {
        let cache = SharedCache::new("resources");
        let key = "private-cluster|private-namespace|private-resource".to_string();
        cache
            .get_or_load(key.clone(), CacheMode::GraceOnly, || async {
                Ok::<_, AppError>(vec!["private-payload".to_string()])
            })
            .await
            .expect("first load");
        cache
            .get_or_load(key.clone(), CacheMode::GraceOnly, || async {
                panic!("hit must not invoke loader");
                #[allow(unreachable_code)]
                Ok::<Vec<String>, AppError>(Vec::new())
            })
            .await
            .expect("cache hit");
        cache.mark_dirty(&key);
        let failed = cache
            .get_or_load(key, CacheMode::GraceOnly, || async {
                Err::<Vec<String>, _>(AppError::cancelled())
            })
            .await;
        assert!(failed.is_err());
        let snapshot = cache.diagnostics();
        assert_eq!(snapshot.label, "resources");
        assert_eq!(snapshot.hits, 1);
        assert_eq!(snapshot.misses, 2);
        assert_eq!(snapshot.joins, 0);
        assert_eq!(snapshot.restored_reload_failures, 1);
        assert_eq!(snapshot.ready, 1);
        assert_eq!(snapshot.dirty, 1);
        assert_eq!(snapshot.loading, 0);
        assert_eq!(snapshot.retained_items, 1);
        assert_eq!(
            snapshot.shallow_payload_bytes,
            std::mem::size_of::<Vec<String>>() + std::mem::size_of::<String>()
        );
        let text = serde_json::to_string(&snapshot).expect("diagnostic fields");
        assert!(!text.contains("private"));
    });
}

#[test]
fn cache_observations_track_ready_only_evictions() {
    tauri::async_runtime::block_on(async {
        let cache = SharedCache::new("resources");
        for index in 0..=MAX_CACHE_ENTRIES {
            cache
                .get_or_load(index.to_string(), CacheMode::GraceOnly, || async {
                    Ok::<_, AppError>(vec![String::new()])
                })
                .await
                .expect("fixture load");
        }
        let snapshot = cache.diagnostics();
        assert_eq!(snapshot.ready, MAX_CACHE_ENTRIES);
        assert_eq!(snapshot.evictions, 1);
        assert_eq!(
            snapshot.misses,
            u64::try_from(MAX_CACHE_ENTRIES + 1).expect("fixture count")
        );
        assert_eq!(snapshot.retained_items, MAX_CACHE_ENTRIES);
    });
}

#[test]
fn cache_observations_count_a_join_and_the_previous_payload_during_reload() {
    tauri::async_runtime::block_on(async {
        let cache = Arc::new(SharedCache::new("resources"));
        cache
            .get_or_load("same".into(), CacheMode::GraceOnly, || async {
                Ok::<_, AppError>(vec![String::new()])
            })
            .await
            .expect("initial payload");
        cache.mark_dirty("same");
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
        let loader_cache = cache.clone();
        let loading = tauri::async_runtime::spawn(async move {
            loader_cache
                .get_or_load("same".into(), CacheMode::GraceOnly, || async move {
                    started_tx.send(()).expect("started receiver");
                    finish_rx.await.expect("fixture completion");
                    Err::<Vec<String>, _>(AppError::cancelled())
                })
                .await
        });
        started_rx.await.expect("loader started");
        let mut joined = Box::pin(cache.get_or_load(
            "same".into(),
            CacheMode::GraceOnly,
            || async { Err::<Vec<String>, _>(AppError::cancelled()) },
        ));
        assert!(joined.as_mut().now_or_never().is_none());
        let snapshot = cache.diagnostics();
        assert_eq!(snapshot.loading, 1);
        assert_eq!(snapshot.ready, 0);
        assert_eq!(snapshot.dirty, 1);
        assert_eq!(snapshot.retained_items, 1);
        assert_eq!(
            snapshot.shallow_payload_bytes,
            std::mem::size_of::<Vec<String>>() + std::mem::size_of::<String>()
        );
        assert_eq!(snapshot.joins, 1);
        assert_eq!(snapshot.misses, 2);
        finish_tx.send(()).expect("fixture loader");
        assert!(loading.await.expect("loader join").is_err());
        assert!(joined.await.is_err());
        let snapshot = cache.diagnostics();
        assert_eq!(snapshot.ready, 1);
        assert_eq!(snapshot.loading, 0);
        assert_eq!(snapshot.dirty, 1);
        assert_eq!(snapshot.retained_items, 1);
        assert_eq!(snapshot.restored_reload_failures, 1);
    });
}
