//! Where a new resource watch starts. Failure modes, written before the code:
//! 1. Another reader refreshes the view's list key at a newer version, and the
//!    view's watch starts there, skipping changes between the two lists.
//! 2. A named namespace served from an all-namespace list reports a version
//!    other than that list's.
//! 3. A view without a version starts anywhere but "0".
//! 4. A 410 at the view's version retries that expired version instead of
//!    resetting to "0" once.
//! 5. A view joins a shared watch that has already sent changes past the
//!    view's rows, and those changes never reach it.
//! 6. An empty start version reaches the API server.
//!
//! The watch starts at whatever version the caller passes, so modes 1 and 2
//! are covered by reading through the store and starting at the version the
//! read returned. A custom kind named like a built-in one is covered where the
//! scope read pairs versions with requests, in `inspection` scope tests.
use super::*;
use crate::models::StreamMessage;
use crate::runtime::Channel;
use crate::{
    commands::{FixedClusterAccess, streams::StreamRegistry},
    test_support::stream_channel,
};
use http::{Request, Response};
use kube::client::Body;
use kubecove_kube::commands::live_store::ListedResources;
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

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

fn modified_at(version: &str) -> Response<Body> {
    let event = json!({
        "type": "MODIFIED",
        "object": {
            "kind": "Pod", "apiVersion": "v1",
            "metadata": { "name": "api", "namespace": "apps", "resourceVersion": version }
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

fn pod_start(namespace: Option<&str>, version: Option<&str>) -> ResourceWatchStart {
    serde_json::from_value(json!({
        "resourceKind": { "kind": "Pod" },
        "namespace": namespace,
        "resourceVersion": version,
    }))
    .unwrap()
}

type Messages = Arc<Mutex<Vec<Value>>>;

fn recording_channel(messages: &Messages) -> Channel<StreamMessage> {
    let messages = Arc::clone(messages);
    stream_channel(move |message| {
        messages.lock().unwrap().push(message);
        Ok(())
    })
}

/// A Pod watch whose API server answers request `index` with `respond(index)`.
struct PodWatch {
    registry: StreamRegistry,
    versions: Arc<Mutex<Vec<String>>>,
    run: tokio::task::JoinHandle<()>,
}

impl PodWatch {
    /// The watch ends at its `reconnects`-th reconnect.
    fn start(
        start: ResourceWatchStart,
        reconnects: usize,
        respond: fn(usize) -> Response<Body>,
    ) -> Self {
        let reconnected = AtomicUsize::new(0);
        let channel = stream_channel(move |message| {
            let ended = message["status"] == "reconnecting"
                && reconnected.fetch_add(1, Ordering::SeqCst) + 1 == reconnects;
            if ended { Err(()) } else { Ok(()) }
        });
        Self::with_channel(start, channel, respond)
    }

    fn with_channel(
        start: ResourceWatchStart,
        channel: Channel<StreamMessage>,
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
        let registry = StreamRegistry::default();
        let (_, broadcaster, started) =
            registry.subscribe_resource("first", "source", CONTEXT, &start, channel);
        assert!(started);
        let access = FixedClusterAccess::new(Client::new(service, "default")).access();
        let run = tokio::spawn(run_resource_watch(
            access,
            CONTEXT.into(),
            start,
            broadcaster,
            ClusterLiveStore::default(),
        ));
        Self {
            registry,
            versions,
            run,
        }
    }

    async fn versions(self) -> Vec<String> {
        self.run.await.unwrap();
        Arc::try_unwrap(self.versions)
            .unwrap()
            .into_inner()
            .unwrap()
    }
}

fn listed(version: &str) -> ListedResources {
    ListedResources {
        rows: Vec::new(),
        resource_version: Some(version.to_string()),
    }
}

/// Reads Pods through the store as a view does and returns the version of the
/// rows the read was served. A fresh `version` list is loaded on a miss.
async fn read_pods(store: &ClusterLiveStore, namespace: Option<&str>, version: &str) -> String {
    let list = listed(version);
    store
        .typed_resources(
            "source".into(),
            CONTEXT.into(),
            "Pod".into(),
            namespace.map(Into::into),
            move || async move { Ok(list) },
        )
        .await
        .unwrap()
        .resource_version
        .expect("listed version")
}

#[tokio::test]
async fn watch_starts_at_the_version_of_the_rows_the_view_shows() {
    let store = ClusterLiveStore::default();
    let shown = read_pods(&store, Some("apps"), "100").await;
    // Another reader's refresh replaces the same list key with a newer list.
    store.mark_watch_resource_dirty(
        "source",
        CONTEXT,
        &pod_start(None, None).key.resource_kind,
        Some("apps"),
    );
    assert_eq!(read_pods(&store, Some("apps"), "200").await, "200");
    // A named namespace is served from a fresh all-namespace list and carries its version.
    let all = read_pods(&store, None, "300").await;
    assert_eq!(read_pods(&store, Some("other"), "unused").await, all);

    for (start, expected) in [
        (pod_start(Some("apps"), Some(&shown)), "100"),
        (pod_start(Some("other"), Some(&all)), "300"),
        (pod_start(Some("apps"), None), "0"),
    ] {
        let versions = PodWatch::start(start, 1, |_| unavailable())
            .versions()
            .await;
        assert_eq!(versions, [expected]);
    }
}

#[tokio::test(start_paused = true)]
async fn expired_start_version_restarts_from_zero_once() {
    let versions = PodWatch::start(pod_start(Some("apps"), Some("100")), 2, |index| {
        if index == 0 { expired() } else { unavailable() }
    })
    .versions()
    .await;
    assert_eq!(versions, ["100", "0"]);
}

fn resyncs(messages: &Messages) -> usize {
    messages
        .lock()
        .unwrap()
        .iter()
        .filter(|message| message["action"] == "resync")
        .count()
}

/// Starts a watch at "100" and waits until its first view receives a message
/// of `kind`, so later joiners see the position that message left.
async fn watch_until(kind: &'static str, respond: fn(usize) -> Response<Body>) -> PodWatch {
    let (seen, wait_seen) = tokio::sync::oneshot::channel();
    let seen = Mutex::new(Some(seen));
    let channel = stream_channel(move |message| {
        if message["type"] == kind {
            seen.lock().unwrap().take().map(|sender| sender.send(()));
        }
        assert_ne!(
            message["action"], "resync",
            "the starting view is at the watch's version"
        );
        Ok(())
    });
    let watch = PodWatch::with_channel(pod_start(Some("apps"), Some("100")), channel, respond);
    wait_seen.await.unwrap();
    watch
}

fn join(watch: &PodWatch, stream: &str, version: Option<&str>) -> usize {
    let messages: Messages = Arc::default();
    let (_, _, started) = watch.registry.subscribe_resource(
        stream,
        "source",
        CONTEXT,
        &pod_start(Some("apps"), version),
        recording_channel(&messages),
    );
    assert!(!started, "{stream} joins the running watch");
    resyncs(&messages)
}

#[tokio::test]
async fn a_view_joining_a_watch_past_its_rows_relists_once() {
    let watch = watch_until("resourceChanged", |index| {
        if index == 0 {
            modified_at("150")
        } else {
            unavailable()
        }
    })
    .await;
    assert_eq!(join(&watch, "behind", Some("100")), 1);
    assert_eq!(join(&watch, "current", Some("150")), 0);
    assert_eq!(join(&watch, "unversioned", None), 0);
    watch.run.abort();
}

#[tokio::test]
async fn after_a_410_reset_every_versioned_joiner_relists() {
    let watch = watch_until(
        "error",
        |index| {
            if index == 0 { expired() } else { unavailable() }
        },
    )
    .await;
    // The reset replayed objects to the first view only.
    assert_eq!(join(&watch, "same-start", Some("100")), 1);
    watch.run.abort();
}

#[test]
fn empty_start_version_is_rejected() {
    let error =
        super::super::validate_resource_watch_scope(CONTEXT, &[pod_start(Some("apps"), Some(" "))])
            .unwrap_err();
    assert_eq!(error.kind, crate::models::AppErrorKind::Validation);
}
