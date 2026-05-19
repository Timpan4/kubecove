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
fn all_namespace_resources_cover_named_namespace_reads() {
    let store = ClusterLiveStore::default();
    tauri::async_runtime::block_on(async {
        store
            .typed_resources("kind-dev".to_string(), "Pod".to_string(), None, || async {
                Ok::<_, AppError>(vec![resource("api", "default"), resource("worker", "jobs")])
            })
            .await
            .expect("all namespace load");

        let default_rows = store
            .typed_resources(
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
