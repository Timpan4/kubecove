use super::*;
use crate::commands::{ClientIntent, FIXED_SOURCE_KEY, FixedClusterAccess};
use http::{Request, Response};
use kube::client::Body;

fn unused_cluster() -> Arc<FixedClusterAccess> {
    let (service, _) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    FixedClusterAccess::new(kube::Client::new(service, "default"))
}

fn pod_request(namespace: Option<&str>) -> ResourceListRequest {
    ResourceListRequest {
        kind: Some("Pod".to_string()),
        resource_kind: None,
        namespace: namespace.map(ToString::to_string),
    }
}

// Failure mode: cancelling a scope leaves shared loaders holding its permits,
// so a later request for the first queued namespace can never acquire one.
#[tokio::test]
async fn cancelled_scope_does_not_strand_a_later_cached_request() {
    use futures_util::FutureExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    let dynamic = DiscoveredResourceKind {
        group: "example.com".into(),
        version: "v1".into(),
        api_version: "example.com/v1".into(),
        kind: "Widget".into(),
        plural: "widgets".into(),
        namespaced: true,
        short_names: Vec::new(),
    };
    for kind in [
        ResourceScopeKind::Typed("Pod".into()),
        ResourceScopeKind::Dynamic(dynamic),
    ] {
        let store = ClusterLiveStore::default();
        let permits = Arc::new(Semaphore::new(MAX_LIST_CONCURRENCY));
        let started = Arc::new(AtomicUsize::new(0));
        let (release, _) = tokio::sync::watch::channel(false);
        let scope = join_all((0..=MAX_LIST_CONCURRENCY).map(|index| {
            let started = Arc::clone(&started);
            let mut wait = release.subscribe();
            cached_scope_resources(
                &store,
                Arc::clone(&permits),
                "scope-cancel".into(),
                "test".into(),
                kind.clone(),
                Some(format!("namespace-{index}")),
                move || async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    wait.wait_for(|released| *released).await.unwrap();
                    Ok(Vec::new())
                },
            )
        }));
        let mut scope = Box::pin(scope);
        assert!(scope.as_mut().now_or_never().is_none());
        assert_eq!(started.load(Ordering::SeqCst), MAX_LIST_CONCURRENCY);
        drop(scope);
        assert!(
            release.send(true).is_err(),
            "abandoned loads release their receivers and permits"
        );

        // Use the existing mock-client test deadline, not a latency budget.
        let rows = tokio::time::timeout(
            Duration::from_secs(5),
            cached_scope_resources(
                &store,
                Arc::new(Semaphore::new(MAX_LIST_CONCURRENCY)),
                "scope-cancel".into(),
                "test".into(),
                kind,
                Some(format!("namespace-{MAX_LIST_CONCURRENCY}")),
                || async { Ok(Vec::new()) },
            ),
        )
        .await
        .expect("cancelled scope must not block a later request")
        .unwrap();
        assert!(rows.rows.is_empty());
    }
}

#[tokio::test]
async fn fetch_error_drives_other_shared_loads_to_completion() {
    use futures_util::FutureExt;
    use tokio::sync::oneshot;

    let store = ClusterLiveStore::default();
    let source = FIXED_SOURCE_KEY;
    let cluster = unused_cluster();
    let mut releases = Vec::new();
    let mut loads = Vec::new();
    // Seed pending shared loads without driving them in another task.
    for namespace in ["a", "b"] {
        let (release, wait) = oneshot::channel::<()>();
        releases.push(release);
        let load = Box::pin(store.typed_resources(
            source.into(),
            "test".into(),
            "Pod".into(),
            Some(namespace.into()),
            move || async move {
                wait.await.unwrap();
                if namespace == "a" {
                    Err(AppError::new("first error", AppErrorKind::Validation))
                } else {
                    Ok(Vec::new())
                }
            },
        ));
        loads.push(load);
        assert!(loads.last_mut().unwrap().as_mut().now_or_never().is_none());
    }
    let access = cluster.access();
    let scope = resource_scope_result_from(
        &access,
        "test".into(),
        vec![pod_request(Some("a")), pod_request(Some("b"))],
        store.clone(),
    );
    futures_util::pin_mut!(scope);
    assert!(scope.as_mut().now_or_never().is_none());
    releases.remove(0).send(()).unwrap();
    assert!(scope.as_mut().now_or_never().is_none());
    releases.remove(0).send(()).unwrap();
    let error = scope.await.err().expect("scope error");
    assert_eq!(error.message, "first error");
    for load in loads {
        let _ = load.await;
    }
    assert_eq!(store.cancel_loading(), 0);
    // A completed second batch must be reusable without calling its loader.
    store
        .typed_resources(
            source.into(),
            "test".into(),
            "Pod".into(),
            Some("b".into()),
            || -> std::future::Ready<Result<ListedResources, AppError>> {
                panic!("completed cache load must be reused")
            },
        )
        .await
        .expect("completed cache load");
}

// Failure modes: a custom kind named like a built-in kind reports the
// built-in list's version; a named namespace served from an all-namespace
// list reports another version than that list's.
#[tokio::test]
async fn each_request_reports_the_version_of_the_list_its_rows_came_from() {
    let custom = DiscoveredResourceKind {
        group: "example.com".into(),
        version: "v1".into(),
        api_version: "example.com/v1".into(),
        kind: "Deployment".into(),
        plural: "deployments".into(),
        namespaced: true,
        short_names: Vec::new(),
    };
    let store = ClusterLiveStore::default();
    let listed = |version: &str| ListedResources {
        rows: Vec::new(),
        resource_version: Some(version.into()),
    };
    let (typed, all, dynamic) = (listed("100"), listed("200"), listed("300"));
    let source = FIXED_SOURCE_KEY.to_string();
    store
        .typed_resources(
            source.clone(),
            "test".into(),
            "Deployment".into(),
            Some("apps".into()),
            || async { Ok(typed) },
        )
        .await
        .unwrap();
    store
        .typed_resources(
            source.clone(),
            "test".into(),
            "Pod".into(),
            None,
            || async { Ok(all) },
        )
        .await
        .unwrap();
    store
        .dynamic_resources(
            source.clone(),
            "test".into(),
            custom.clone(),
            Some("apps".into()),
            || async { Ok(dynamic) },
        )
        .await
        .unwrap();
    store
        .flux_ownership_index(source, "test".into(), || async {
            Ok(crate::commands::helpers::FluxOwnershipIndex::default())
        })
        .await
        .unwrap();
    let custom_request = ResourceListRequest {
        kind: None,
        resource_kind: Some(custom),
        namespace: Some("apps".into()),
    };
    let response = resource_scope_result_from(
        &unused_cluster().access(),
        "test".into(),
        vec![pod_request(Some("apps")), custom_request],
        store,
    )
    .await
    .unwrap();
    let mut versions: Vec<_> = response
        .resource_versions
        .iter()
        .map(|entry| {
            let kind = entry.request.kind.clone().or_else(|| {
                entry
                    .request
                    .resource_kind
                    .as_ref()
                    .map(|kind| kind.api_version.clone())
            });
            (kind.unwrap(), entry.resource_version.as_str())
        })
        .collect();
    versions.sort();
    assert_eq!(
        versions,
        [
            ("Pod".to_string(), "200"),
            ("example.com/v1".to_string(), "300")
        ]
    );
}

#[tokio::test]
async fn empty_requests_return_without_requesting_a_client() {
    let cluster = unused_cluster();
    let result = resource_scope_from(
        &cluster.access(),
        "missing-context".to_string(),
        Vec::new(),
        ClusterLiveStore::default(),
    )
    .await;

    assert!(result.expect("empty scope").is_empty());
    assert_eq!(cluster.intents(), Vec::<ClientIntent>::new());
}

#[test]
fn groups_multiple_namespaces_for_one_kind_without_promoting_to_all() {
    let groups = group_requests(vec![
        pod_request(Some("default")),
        pod_request(Some("argocd")),
    ])
    .expect("groups");
    let group = groups.get("typed:Pod").expect("pod group");

    assert!(!should_promote_to_all(group));
    assert_eq!(
        fetch_namespaces(group),
        vec![Some("argocd".to_string()), Some("default".to_string())]
    );
    assert_eq!(group.namespaces.len(), 2);
}

#[test]
fn keeps_large_namespace_sets_namespace_scoped() {
    let groups = group_requests(vec![
        pod_request(Some("default")),
        pod_request(Some("argocd")),
        pod_request(Some("jobs")),
        pod_request(Some("kube-system")),
        pod_request(Some("monitoring")),
        pod_request(Some("platform")),
        pod_request(Some("staging")),
        pod_request(Some("prod")),
    ])
    .expect("groups");
    let group = groups.get("typed:Pod").expect("pod group");

    assert!(!should_promote_to_all(group));
    assert_eq!(
        fetch_namespaces(group),
        vec![
            Some("argocd".to_string()),
            Some("default".to_string()),
            Some("jobs".to_string()),
            Some("kube-system".to_string()),
            Some("monitoring".to_string()),
            Some("platform".to_string()),
            Some("prod".to_string()),
            Some("staging".to_string()),
        ]
    );
}

#[test]
fn promotes_explicit_all_namespace_request_to_one_fetch() {
    let groups = group_requests(vec![pod_request(None)]).expect("groups");
    let group = groups.get("typed:Pod").expect("pod group");

    assert!(should_promote_to_all(group));
    assert_eq!(fetch_namespaces(group), vec![None]);
}

#[test]
fn all_namespace_group_keeps_ownership_read_cluster_wide() {
    let groups =
        group_requests(vec![pod_request(None), pod_request(Some("default"))]).expect("groups");

    assert_eq!(ownership_namespaces(&groups).len(), 0);
}

#[test]
fn rejects_requests_without_one_kind_identity() {
    let err = request_kind(&ResourceListRequest {
        kind: None,
        resource_kind: None,
        namespace: None,
    })
    .expect_err("missing kind");

    assert_eq!(err.kind, AppErrorKind::Validation);
}
