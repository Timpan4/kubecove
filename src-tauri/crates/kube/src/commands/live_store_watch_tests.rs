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
        for event in [
            WatchEvent::Modified(pod("a", "Running", "11")),
            WatchEvent::Added(pod("c", "Pending", "12")),
            WatchEvent::Deleted(pod("b", "Running", "13")),
            WatchEvent::Modified(pod("c", "Failed", "14")),
        ] {
            store.apply_watch_event(SOURCE, CONTEXT, &pods(), &event);
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
        &WatchEvent::Modified(pod("a", "Running", "11")),
    );
    release.send(()).unwrap();
    in_flight.await.unwrap();
    let snapshot = list(&store, None, relisted).await;
    assert_eq!(statuses(&snapshot), [("fresh".into(), "Running".into())]);
}
