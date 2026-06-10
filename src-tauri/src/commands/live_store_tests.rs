use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn resource(name: &str, namespace: &str) -> ResourceSummary {
    ResourceSummary {
        kind: "Pod".to_string(),
        cluster: "kind-dev".to_string(),
        name: name.to_string(),
        namespace: Some(namespace.to_string()),
        age: "1m".to_string(),
        api_version: None,
        group: None,
        version: None,
        plural: None,
        namespaced: None,
        dynamic: None,
        created_at: None,
        status: None,
        ready: None,
        restarts: None,
        owner_ref: None,
        argo_app: None,
        helm_release: None,
        git_ops_owner: None,
    }
}

#[test]
fn grace_cache_reuses_recent_completed_value() {
    tauri::async_runtime::block_on(async {
        let cache = SharedCache::new("test");
        let loads = Arc::new(AtomicUsize::new(0));

        let first_loads = loads.clone();
        let first = cache
            .get_or_load(
                "same".to_string(),
                CacheMode::GraceOnly,
                move || async move {
                    first_loads.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, AppError>(vec!["one".to_string()])
                },
            )
            .await
            .expect("first load");
        let second_loads = loads.clone();
        let second = cache
            .get_or_load(
                "same".to_string(),
                CacheMode::GraceOnly,
                move || async move {
                    second_loads.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, AppError>(vec!["two".to_string()])
                },
            )
            .await
            .expect("second load");

        assert_eq!(first, vec!["one".to_string()]);
        assert_eq!(second, vec!["one".to_string()]);
        assert_eq!(loads.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn dirty_live_cache_reloads_even_inside_grace_window() {
    tauri::async_runtime::block_on(async {
        let cache = SharedCache::new("test");
        let loads = Arc::new(AtomicUsize::new(0));

        let first_loads = loads.clone();
        let first = cache
            .get_or_load(
                "same".to_string(),
                CacheMode::LiveFor(Duration::from_secs(30)),
                move || async move {
                    first_loads.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, AppError>(vec!["one".to_string()])
                },
            )
            .await
            .expect("first load");

        cache.mark_dirty("same");

        let second_loads = loads.clone();
        let second = cache
            .get_or_load(
                "same".to_string(),
                CacheMode::LiveFor(Duration::from_secs(30)),
                move || async move {
                    second_loads.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, AppError>(vec!["two".to_string()])
                },
            )
            .await
            .expect("second load");

        assert_eq!(first, vec!["one".to_string()]);
        assert_eq!(second, vec!["two".to_string()]);
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn live_cache_reloads_after_freshness_window() {
    tauri::async_runtime::block_on(async {
        let cache = SharedCache::new("test");
        let loads = Arc::new(AtomicUsize::new(0));

        let first_loads = loads.clone();
        let first = cache
            .get_or_load(
                "same".to_string(),
                CacheMode::LiveFor(Duration::from_millis(1)),
                move || async move {
                    first_loads.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, AppError>(vec!["one".to_string()])
                },
            )
            .await
            .expect("first load");

        std::thread::sleep(Duration::from_millis(5));

        let second_loads = loads.clone();
        let second = cache
            .get_or_load(
                "same".to_string(),
                CacheMode::LiveFor(Duration::from_millis(1)),
                move || async move {
                    second_loads.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, AppError>(vec!["two".to_string()])
                },
            )
            .await
            .expect("second load");

        assert_eq!(first, vec!["one".to_string()]);
        assert_eq!(second, vec!["two".to_string()]);
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn all_namespace_resources_cover_named_namespace_reads() {
    let store = ClusterLiveStore::default();
    tauri::async_runtime::block_on(async {
        store
            .typed_resources(
                "kubeconfigEnv=KUBECONFIG".to_string(),
                "kind-dev".to_string(),
                "Pod".to_string(),
                None,
                || async {
                    Ok::<_, AppError>(vec![resource("api", "default"), resource("worker", "jobs")])
                },
            )
            .await
            .expect("all namespace load");

        let default_rows = store
            .typed_resources(
                "kubeconfigEnv=KUBECONFIG".to_string(),
                "kind-dev".to_string(),
                "Pod".to_string(),
                Some("default".to_string()),
                || async { Ok::<_, AppError>(Vec::new()) },
            )
            .await
            .expect("covered namespace read");

        assert_eq!(default_rows.len(), 1);
        assert_eq!(default_rows[0].name, "api");
    });
}

#[test]
fn normalized_builtin_watch_invalidates_typed_resource_cache() {
    let store = ClusterLiveStore::default();
    tauri::async_runtime::block_on(async {
        store
            .typed_resources(
                "kubeconfigEnv=KUBECONFIG".to_string(),
                "kind-dev".to_string(),
                "Pod".to_string(),
                Some("default".to_string()),
                || async { Ok::<_, AppError>(vec![resource("api", "default")]) },
            )
            .await
            .expect("typed load");

        store.mark_watch_resource_dirty(
            "kubeconfigEnv=KUBECONFIG",
            "kind-dev",
            &WatchResourceKind {
                kind: "Pod".to_string(),
                group: Some(String::new()),
                version: Some("v1".to_string()),
                api_version: Some("v1".to_string()),
                plural: Some("pods".to_string()),
                namespaced: Some(true),
            },
            Some("default"),
        );

        let rows = store
            .typed_resources(
                "kubeconfigEnv=KUBECONFIG".to_string(),
                "kind-dev".to_string(),
                "Pod".to_string(),
                Some("default".to_string()),
                || async { Ok::<_, AppError>(vec![resource("worker", "default")]) },
            )
            .await
            .expect("typed reload");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "worker");
    });
}

#[test]
fn ready_cache_entries_are_bounded() {
    tauri::async_runtime::block_on(async {
        let cache = SharedCache::new("test");

        for index in 0..(MAX_CACHE_ENTRIES + 24) {
            cache
                .get_or_load(
                    format!("key-{index}"),
                    CacheMode::LiveFor(Duration::from_secs(30)),
                    move || async move { Ok::<_, AppError>(vec![index]) },
                )
                .await
                .expect("cache load");
        }

        assert_eq!(cache.len(), MAX_CACHE_ENTRIES);
        assert_eq!(cache.ready_len(), MAX_CACHE_ENTRIES);
    });
}

#[test]
fn ready_cache_budget_keeps_dirty_entries_over_clean_entries() {
    tauri::async_runtime::block_on(async {
        let cache = SharedCache::new("test");

        cache
            .get_or_load(
                "dirty".to_string(),
                CacheMode::LiveFor(Duration::from_secs(30)),
                || async { Ok::<_, AppError>(vec![0]) },
            )
            .await
            .expect("dirty load");
        cache.mark_dirty("dirty");

        for index in 0..(MAX_CACHE_ENTRIES + 24) {
            cache
                .get_or_load(
                    format!("clean-{index}"),
                    CacheMode::LiveFor(Duration::from_secs(30)),
                    move || async move { Ok::<_, AppError>(vec![index]) },
                )
                .await
                .expect("clean load");
        }

        assert_eq!(cache.ready_len(), MAX_CACHE_ENTRIES);
        assert!(cache.has_key("dirty"));
    });
}

#[test]
fn ready_cache_budget_evicts_oldest_clean_entries_first() {
    tauri::async_runtime::block_on(async {
        let cache = SharedCache::new("test");

        for index in 0..(MAX_CACHE_ENTRIES + 2) {
            cache
                .get_or_load(
                    format!("key-{index}"),
                    CacheMode::LiveFor(Duration::from_secs(30)),
                    move || async move { Ok::<_, AppError>(vec![index]) },
                )
                .await
                .expect("cache load");
            std::thread::sleep(Duration::from_millis(1));
        }

        assert_eq!(cache.ready_len(), MAX_CACHE_ENTRIES);
        assert!(!cache.has_key("key-0"));
        assert!(!cache.has_key("key-1"));
        assert!(cache.has_key(&format!("key-{}", MAX_CACHE_ENTRIES + 1)));
    });
}
