//! Where a new resource watch starts. Failure modes, written before the code:
//! 1. With no cached list the watch must still converge, so it starts at "0".
//! 2. An event between the view's list and the watch start is lost unless the
//!    watch starts at that list's version.
//! 3. A 410 for the cached version must reset to "0" once, never retry the
//!    expired cached version.
//! 4. A dirty, evicted or dirtied-while-loading list must not hand its older
//!    version to a new watch.
//! 5. A watch must not start past the rows the view shows: while the view's
//!    list loads, the watch waits for that load instead of a later one, and a
//!    cancelled load starts the watch at "0".
use super::*;
use crate::{
    commands::{FixedClusterAccess, streams::StreamRegistry},
    test_support::stream_channel,
};
use http::{Request, Response};
use kube::client::Body;
use kubecove_kube::commands::live_store::ListedResources;
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::oneshot;

const CONTEXT: &str = "dev";

fn unavailable() -> Response<Body> {
    let status = json!({
        "kind": "Status", "apiVersion": "v1", "status": "Failure",
        "reason": "ServiceUnavailable", "message": "watch unavailable", "code": 503
    });
    Response::builder()
        .status(503)
        .body(Body::from(status.to_string().into_bytes()))
        .unwrap()
}

fn expired() -> Response<Body> {
    let event = json!({
        "type": "ERROR",
        "object": {
            "kind": "Status", "apiVersion": "v1", "status": "Failure",
            "reason": "Expired", "message": "too old resource version", "code": 410
        }
    });
    Response::builder()
        .body(Body::from(format!("{event}\n").into_bytes()))
        .unwrap()
}

fn requested_version(request: &Request<Body>) -> String {
    request
        .uri()
        .query()
        .unwrap_or_default()
        .split('&')
        .find_map(|pair| pair.strip_prefix("resourceVersion="))
        .unwrap_or_default()
        .to_string()
}

struct PodWatch {
    versions: Arc<Mutex<Vec<String>>>,
    run: tokio::task::JoinHandle<()>,
}

impl PodWatch {
    /// Starts a Pod watch whose API server answers each request with
    /// `respond(index)`. The watch ends at its `reconnects`-th reconnect.
    fn start(
        store: &ClusterLiveStore,
        namespace: Option<&str>,
        reconnects: usize,
        respond: fn(usize) -> Response<Body>,
    ) -> Self {
        let versions = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&versions);
        let service = tower::service_fn(move |request: Request<Body>| {
            let mut versions = recorded.lock().unwrap();
            versions.push(requested_version(&request));
            let response = respond(versions.len() - 1);
            std::future::ready(Ok::<_, std::convert::Infallible>(response))
        });
        let reconnected = AtomicUsize::new(0);
        let channel = stream_channel(move |message| {
            let ended = message["status"] == "reconnecting"
                && reconnected.fetch_add(1, Ordering::SeqCst) + 1 == reconnects;
            if ended { Err(()) } else { Ok(()) }
        });
        let key: WatchResourceKey = serde_json::from_value(
            json!({ "resourceKind": { "kind": "Pod" }, "namespace": namespace }),
        )
        .unwrap();
        let registry = StreamRegistry::default();
        let (_, broadcaster, _) =
            registry.subscribe_resource("test", "source", CONTEXT, &key, channel);
        let access = FixedClusterAccess::new(Client::new(service, "default")).access();
        let run = tokio::spawn(run_resource_watch(
            access,
            CONTEXT.into(),
            key,
            broadcaster,
            store.clone(),
        ));
        Self { versions, run }
    }

    async fn versions(self) -> Vec<String> {
        self.run.await.unwrap();
        Arc::try_unwrap(self.versions)
            .unwrap()
            .into_inner()
            .unwrap()
    }
}

fn source_key() -> String {
    let (service, _) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    FixedClusterAccess::new(Client::new(service, "default"))
        .access()
        .source_key()
        .to_string()
}

fn pod_kind() -> crate::models::WatchResourceKind {
    serde_json::from_value(json!({ "kind": "Pod" })).unwrap()
}

fn listed(version: &str) -> ListedResources {
    ListedResources {
        rows: Vec::new(),
        resource_version: Some(version.to_string()),
    }
}

async fn cache_pods(store: &ClusterLiveStore, namespace: Option<&str>, version: &str) {
    let list = listed(version);
    store
        .typed_resources(
            source_key(),
            CONTEXT.into(),
            "Pod".into(),
            namespace.map(Into::into),
            move || async move { Ok(list) },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn watch_starts_at_the_list_the_view_was_served() {
    // (cached scope, watched namespace, expected start)
    let cases = [
        (None, Some("apps"), "0"),
        (Some(Some("apps")), Some("apps"), "100"),
        // A named namespace is served from a fresh all-namespace list.
        (Some(None), Some("apps"), "100"),
        (Some(None), None, "100"),
        (Some(Some("other")), Some("apps"), "0"),
    ];
    for (cached, watched, expected) in cases {
        let store = ClusterLiveStore::default();
        if let Some(namespace) = cached {
            cache_pods(&store, namespace, "100").await;
        }
        let versions = PodWatch::start(&store, watched, 1, |_| unavailable())
            .versions()
            .await;
        assert_eq!(
            versions,
            [expected],
            "cached {cached:?}, watched {watched:?}"
        );
    }
}

#[tokio::test]
async fn stale_lists_do_not_seed_a_watch() {
    for evict in [false, true] {
        let store = ClusterLiveStore::default();
        cache_pods(&store, Some("apps"), "100").await;
        let kind = pod_kind();
        if evict {
            let key = WatchResourceKey {
                resource_kind: kind,
                namespace: Some("apps".into()),
            };
            store.refresh_view(&source_key(), CONTEXT, &[key], &["apps".into()]);
        } else {
            store.mark_watch_resource_dirty(&source_key(), CONTEXT, &kind, Some("apps"));
        }
        let versions = PodWatch::start(&store, Some("apps"), 1, |_| unavailable())
            .versions()
            .await;
        assert_eq!(versions, ["0"], "evict {evict}");
    }
}

#[tokio::test(start_paused = true)]
async fn expired_cached_version_restarts_from_zero_once() {
    let store = ClusterLiveStore::default();
    cache_pods(&store, Some("apps"), "100").await;
    let versions = PodWatch::start(&store, Some("apps"), 2, |index| {
        if index == 0 { expired() } else { unavailable() }
    })
    .versions()
    .await;
    assert_eq!(versions, ["100", "0"]);
}

/// A view's list that is still loading when its watch starts.
struct PendingList {
    release: oneshot::Sender<()>,
    read: tokio::task::JoinHandle<()>,
}

impl PendingList {
    async fn start(store: &ClusterLiveStore, version: &'static str) -> Self {
        let (release, gate) = oneshot::channel::<()>();
        let reader = store.clone();
        let read = tokio::spawn(async move {
            let _ = reader
                .typed_resources(
                    source_key(),
                    CONTEXT.into(),
                    "Pod".into(),
                    Some("apps".into()),
                    move || async move {
                        gate.await.unwrap();
                        Ok(listed(version))
                    },
                )
                .await;
        });
        // Let the read register its load before the watch looks it up.
        tokio::task::yield_now().await;
        Self { release, read }
    }
}

#[tokio::test]
async fn watch_waits_for_the_list_in_flight() {
    // How the in-flight list ends: released clean, dirtied first, or abandoned.
    for (outcome, expected) in [("clean", "300"), ("dirty", "0"), ("abandoned", "0")] {
        let store = ClusterLiveStore::default();
        let list = PendingList::start(&store, "300").await;
        let watch = PodWatch::start(&store, Some("apps"), 1, |_| unavailable());
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }
        assert!(
            watch.versions.lock().unwrap().is_empty(),
            "watch must wait for the list"
        );
        match outcome {
            "dirty" => {
                store.mark_watch_resource_dirty(&source_key(), CONTEXT, &pod_kind(), Some("apps"));
                list.release.send(()).unwrap();
            }
            "abandoned" => list.read.abort(),
            _ => list.release.send(()).unwrap(),
        }
        assert_eq!(watch.versions().await, [expected], "{outcome}");
    }
}
