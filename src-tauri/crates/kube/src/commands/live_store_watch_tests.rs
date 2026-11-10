use super::*;
use crate::models::AppError;
use serde_json::json;

const SOURCE: &str = "source";
const CONTEXT: &str = "dev";

fn summary(name: &str, namespace: &str, status: &str) -> ResourceSummary {
    serde_json::from_value(json!({
        "kind": "Pod", "cluster": CONTEXT, "name": name, "namespace": namespace,
        "age": "1m", "status": status
    }))
    .unwrap()
}

/// A typed projection that reads the phase as the status. Pods named
/// "opaque" have no projection.
fn project(_kind: &str, _context: &str, object: &DynamicObject) -> Option<ResourceSummary> {
    let name = object.metadata.name.as_deref()?;
    (name != "opaque").then(|| {
        summary(
            name,
            object.metadata.namespace.as_deref().unwrap_or_default(),
            object.data["status"]["phase"].as_str().unwrap_or_default(),
        )
    })
}

fn pod(name: &str, phase: &str, version: &str) -> DynamicObject {
    serde_json::from_value(json!({
        "apiVersion": "v1", "kind": "Pod",
        "metadata": {"name": name, "namespace": "apps", "resourceVersion": version},
        "status": {"phase": phase}
    }))
    .unwrap()
}

fn pods() -> WatchResourceKind {
    WatchResourceKind {
        kind: "Pod".into(),
        group: Some(String::new()),
        version: Some("v1".into()),
        api_version: Some("v1".into()),
        plural: Some("pods".into()),
        namespaced: Some(true),
    }
}

/// A watch of `namespace` (`None` for all) that has delivered every change
/// after `position`.
fn origin<'a>(namespace: Option<&'a str>, position: Option<&'a str>) -> WatchOrigin<'a> {
    WatchOrigin {
        namespace,
        position,
    }
}

/// Reads the pods of `namespace`, listing `loaded` at version "10" on a miss.
async fn list(
    store: &ClusterLiveStore,
    namespace: Option<&str>,
    loaded: Vec<ResourceSummary>,
) -> ListSnapshot {
    store
        .typed_resources(
            SOURCE.into(),
            CONTEXT.into(),
            "Pod".into(),
            namespace.map(str::to_owned),
            move || async move {
                Ok::<_, AppError>(super::super::ListedResources {
                    rows: loaded,
                    resource_version: Some("10".into()),
                })
            },
        )
        .await
        .unwrap()
}

fn statuses(snapshot: &ListSnapshot) -> Vec<(String, String)> {
    snapshot
        .rows
        .iter()
        .map(|row| (row.name.clone(), row.status.clone().unwrap_or_default()))
        .collect()
}

// Failure modes: a modified object leaves its cached row stale or lists the
// kind again; an added object is missing; a deleted object lingers; an event
// after a delete updates the wrong row; the namespace list and the
// all-namespace list disagree; the list version stays behind the applied event.
#[tokio::test]
async fn watch_events_update_cached_lists_without_listing_again() {
    // A namespace read uses a cached all-namespace list, so each list gets its own store.
    for namespace in [None, Some("apps")] {
        let store = ClusterLiveStore::default().applying_watch_rows(project);
        let seed = vec![
            summary("a", "apps", "Pending"),
            summary("b", "apps", "Running"),
        ];
        list(&store, namespace, seed).await;
        // Each event's watch position is the previous event's version.
        for (event, position) in [
            (WatchEvent::Modified(pod("a", "Running", "11")), "10"),
            (WatchEvent::Added(pod("c", "Pending", "12")), "11"),
            (WatchEvent::Deleted(pod("b", "Running", "13")), "12"),
            (WatchEvent::Modified(pod("c", "Failed", "14")), "13"),
        ] {
            store.apply_watch_event(
                SOURCE,
                CONTEXT,
                &pods(),
                origin(namespace, Some(position)),
                &event,
            );
        }
        let snapshot = list(&store, namespace, Vec::new()).await;
        assert_eq!(
            statuses(&snapshot),
            [
                ("a".into(), "Running".into()),
                ("c".into(), "Failed".into())
            ],
            "{namespace:?}"
        );
        assert_eq!(snapshot.resource_version.as_deref(), Some("14"));
    }
}

// Failure modes: a second watch of the same list that runs behind the first
// (or a list read after an event) rolls a row back, brings back a deleted
// object, or moves the list version backwards.
#[tokio::test]
async fn events_the_list_already_holds_are_skipped() {
    let store = ClusterLiveStore::default().applying_watch_rows(project);
    let seed = vec![
        summary("a", "apps", "Pending"),
        summary("b", "apps", "Running"),
    ];
    list(&store, None, seed).await;
    for (event, position) in [
        (WatchEvent::Modified(pod("b", "Failed", "11")), "10"),
        (WatchEvent::Deleted(pod("b", "Failed", "12")), "11"),
        (WatchEvent::Modified(pod("a", "Running", "13")), "12"),
        // The lagging watch, and an event older than the list.
        (WatchEvent::Modified(pod("b", "Failed", "11")), "10"),
        (WatchEvent::Modified(pod("a", "Pending", "9")), "8"),
    ] {
        store.apply_watch_event(
            SOURCE,
            CONTEXT,
            &pods(),
            origin(None, Some(position)),
            &event,
        );
    }
    let snapshot = list(&store, None, Vec::new()).await;
    assert_eq!(statuses(&snapshot), [("a".into(), "Running".into())]);
    assert_eq!(snapshot.resource_version.as_deref(), Some("13"));
}

// Failure modes: a namespace watch advances the all-namespace list past
// changes in other namespaces; a watch that started after the list (or replays
// from "0") patches it across changes it never delivered. Each must make the
// next read list again.
#[tokio::test]
async fn events_that_do_not_follow_the_list_make_the_next_read_list_again() {
    let relisted = vec![summary("fresh", "apps", "Running")];
    for (list_namespace, watch) in [
        (None, origin(Some("apps"), Some("10"))),
        (Some("apps"), origin(None, Some("15"))),
        (Some("apps"), origin(None, None)),
    ] {
        let store = ClusterLiveStore::default().applying_watch_rows(project);
        list(&store, list_namespace, vec![summary("a", "apps", "Pending")]).await;
        store.apply_watch_event(
            SOURCE,
            CONTEXT,
            &pods(),
            watch,
            &WatchEvent::Modified(pod("a", "Running", "16")),
        );
        let snapshot = list(&store, list_namespace, relisted.clone()).await;
        assert_eq!(
            statuses(&snapshot),
            [("fresh".into(), "Running".into())],
            "{list_namespace:?} {watch:?}"
        );
    }
}

// Failure modes: an event the store cannot project, or one that arrives while
// a list is in flight, is lost instead of forcing the next read to list again;
// a store built without watch rows (the web's) stops listing again.
#[tokio::test]
async fn unappliable_watch_events_make_the_next_read_list_again() {
    let relisted = vec![summary("fresh", "apps", "Running")];
    for store in [
        ClusterLiveStore::default(),
        ClusterLiveStore::default().applying_watch_rows(project),
    ] {
        list(&store, None, vec![summary("a", "apps", "Pending")]).await;
        store.apply_watch_event(
            SOURCE,
            CONTEXT,
            &pods(),
            origin(None, Some("10")),
            &WatchEvent::Modified(pod("opaque", "Running", "11")),
        );
        let snapshot = list(&store, None, relisted.clone()).await;
        assert_eq!(statuses(&snapshot), [("fresh".into(), "Running".into())]);
    }

    let store = ClusterLiveStore::default().applying_watch_rows(project);
    let (release, wait) = tokio::sync::oneshot::channel::<()>();
    let loading = store.clone();
    let in_flight = tokio::spawn(async move {
        loading
            .typed_resources(
                SOURCE.into(),
                CONTEXT.into(),
                "Pod".into(),
                None,
                move || async move {
                    wait.await.unwrap();
                    Ok::<_, AppError>(vec![summary("a", "apps", "Pending")])
                },
            )
            .await
            .unwrap()
    });
    tokio::task::yield_now().await;
    store.apply_watch_event(
        SOURCE,
        CONTEXT,
        &pods(),
        origin(None, Some("10")),
        &WatchEvent::Modified(pod("a", "Running", "11")),
    );
    release.send(()).unwrap();
    in_flight.await.unwrap();
    let snapshot = list(&store, None, relisted).await;
    assert_eq!(statuses(&snapshot), [("fresh".into(), "Running".into())]);
}
