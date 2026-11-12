use super::*;
use kubecove_gpui::backend::WatchTarget;
use kubecove_lib::models::{StreamMessage, StreamStatus, WatchResourceTarget};
use tokio::sync::mpsc;

#[gpui_kit::test]
fn cancelling_an_opening_watch_releases_its_stream(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.live.gate.lock().unwrap() = Some(receiver);
    let handle = open(cx, backend.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        sender.send(()).is_err(),
        "leaving Overview cancels its opening watch"
    );
}

#[gpui_kit::test]
fn ended_streams_restart_on_the_web_recovery_tick(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    let old = backend
        .live
        .streams
        .lock()
        .unwrap()
        .iter()
        .map(|(_, tx)| tx.clone())
        .collect::<Vec<_>>();
    backend.live.send(StreamMessage::Stopped {
        stream_id: "native".into(),
    });
    cx.run_until_parked();
    // resource-watch.ts::RESOURCE_RECOVERY_INTERVAL.
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(15_000));
    cx.run_until_parked();
    assert!(old.iter().all(mpsc::UnboundedSender::is_closed));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("live-label").label(), Some("Live"));
    })
    .unwrap();
}

#[derive(Default)]
pub(super) struct LiveControls {
    pub events_fail: AtomicBool,
    /// Whether opening a watch fails.
    open_fails: AtomicBool,
    gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    streams: Mutex<Vec<(WatchTarget, mpsc::UnboundedSender<StreamMessage>)>>,
    /// Whether the backend reads watch start versions, and whether the read fails.
    start_versions: Option<bool>,
    /// The order in which start-version reads settle and scope reads start.
    pub reads: Arc<Mutex<Vec<&'static str>>>,
    /// The list versions a scope read reports when it returns.
    pub list_versions: Arc<Mutex<kubecove_gpui::backend::ListVersions>>,
    /// The versions each scope read returned, in the order they returned.
    pub returned_lists: Arc<Mutex<Vec<kubecove_gpui::backend::ListVersions>>>,
}

const START_VERSION: &str = "42";

impl LiveControls {
    pub fn start_version(&self) -> Option<Read<String>> {
        let fails = self.start_versions?;
        let reads = self.reads.clone();
        // Settles on its second poll and logs then, so a list that does not
        // wait for it logs first.
        Some(Box::pin(async move {
            let mut polled = false;
            std::future::poll_fn(|cx| {
                if std::mem::replace(&mut polled, true) {
                    return std::task::Poll::Ready(());
                }
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            })
            .await;
            reads.lock().unwrap().push("version");
            if fails {
                return Err(AppError::new("list denied", AppErrorKind::Forbidden));
            }
            Ok(START_VERSION.into())
        }))
    }
    fn resource_watch_versions(&self) -> Vec<Option<String>> {
        self.streams
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(target, tx)| match target {
                WatchTarget::Resource {
                    resource_version, ..
                } if !tx.is_closed() => Some(resource_version.clone()),
                _ => None,
            })
            .collect()
    }
}

fn label(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) -> Option<String> {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.find("live-label").label().map(str::to_owned)
    })
    .unwrap()
}

fn shows_rows(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) -> bool {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window
            .try_find("resource:Deployment:payments:payments-api:mock-other")
            .is_some()
    })
    .unwrap()
}

/// Opens the other context with its rows held, and returns their release.
fn load_other_context(
    cx: &mut TestAppContext,
    backend: &Controlled,
    handle: gpui_kit::AnyWindowHandle,
) -> tokio::sync::oneshot::Sender<()> {
    let (release, pending) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(pending);
    backend.live.reads.lock().unwrap().clear();
    cx.update_window(handle, |_, window, cx| {
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    release
}

// Failure modes: the watches wait for the rows again; a watch opens without
// its start version, which replays every object; the rows are listed before
// the versions are read, so changes between the list and the watch are lost;
// a failed version read leaves the watch waiting for good.
#[gpui_kit::test]
fn watches_open_at_their_start_version_while_rows_load(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled {
        live: LiveControls {
            start_versions: Some(false),
            ..Default::default()
        },
        ..Default::default()
    });
    let handle = open(cx, backend.clone());
    let release = load_other_context(cx, &backend, handle);
    // The page shows "Loading resources" without the Live pill until the rows
    // arrive, so the open watches are what this state shows.
    assert!(!shows_rows(cx, handle), "the rows are still loading");
    let versions = backend.live.resource_watch_versions();
    assert!(!versions.is_empty());
    assert!(
        versions
            .iter()
            .all(|version| version.as_deref() == Some(START_VERSION)),
        "{versions:?}"
    );
    let reads = backend.live.reads.lock().unwrap().clone();
    let first_list = reads.iter().position(|read| *read == "list");
    assert!(
        first_list
            .is_some_and(|list| reads[..list].iter().all(|read| *read == "version") && list > 0),
        "every version is read before the rows are listed: {reads:?}"
    );
    assert!(
        !reads[first_list.unwrap()..].contains(&"version"),
        "{reads:?}"
    );
    release.send(()).unwrap();
    cx.run_until_parked();
    assert!(shows_rows(cx, handle));
    assert_eq!(label(cx, handle).as_deref(), Some("Live"));
}

// Failure mode: a change arrives while the rows are listed, and its reload
// returns before the list. The list is newer, but the reload's patch waits for
// it and then overwrites it, so a row the list shows disappears.
#[gpui_kit::test]
fn a_reload_older_than_the_list_leaves_the_listed_rows(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled {
        live: LiveControls {
            start_versions: Some(false),
            ..Default::default()
        },
        ..Default::default()
    });
    let handle = open(cx, backend.clone());
    let release = load_other_context(cx, &backend, handle);
    let signature = backend
        .live
        .streams
        .lock()
        .unwrap()
        .iter()
        .find_map(|(target, tx)| match target {
            WatchTarget::Resource { key, .. }
                if !tx.is_closed() && key.resource_kind.kind == "Deployment" =>
            {
                Some(kubecove_gpui::backend::watch_signature(key))
            }
            _ => None,
        })
        .expect("the Deployment watch opened while the rows load");
    let set_version = |version: &str| {
        let mut versions = backend.live.list_versions.lock().unwrap();
        versions.insert(signature.clone(), version.into());
    };
    // The reload reads the kind at version 43, when the Deployment is gone.
    backend.live.returned_lists.lock().unwrap().clear();
    set_version("43");
    backend.sessions.enabled.store(true, Ordering::SeqCst);
    backend.live.send(StreamMessage::ResourceChanged {
        stream_id: "native".into(),
        target: WatchResourceTarget {
            cluster: "mock-other".into(),
            kind: "Deployment".into(),
            namespace: Some("payments".into()),
            name: Some("payments-api".into()),
        },
        action: "deleted".into(),
    });
    cx.run_until_parked();
    // live_data.rs::COALESCE, the pause after each reload.
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(250));
    cx.run_until_parked();
    // The reloads returned before the held list, so their patches wait for it.
    // The notice reaches every open watch, so each kind reloads.
    let returned: Vec<_> = backend
        .live
        .returned_lists
        .lock()
        .unwrap()
        .iter()
        .map(|versions| versions.get(&signature).cloned())
        .collect();
    assert!(
        !returned.is_empty()
            && returned
                .iter()
                .all(|version| version.as_deref() == Some("43")),
        "only reloads at 43 returned while the list was held: {returned:?}"
    );
    // The held list, read before the flag changed, shows it again at 44.
    backend.sessions.enabled.store(false, Ordering::SeqCst);
    set_version("44");
    release.send(()).unwrap();
    cx.run_until_parked();
    assert!(shows_rows(cx, handle), "the newer list's row stays");
}

// Failure mode: a watch that fails to open while the rows load reopens on the
// recovery tick without its start version, because the rows have none yet,
// and replays every object of its kind.
#[gpui_kit::test]
fn a_watch_reopened_while_rows_load_keeps_its_start_version(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled {
        live: LiveControls {
            start_versions: Some(false),
            ..Default::default()
        },
        ..Default::default()
    });
    let handle = open(cx, backend.clone());
    backend.live.open_fails.store(true, Ordering::SeqCst);
    let release = load_other_context(cx, &backend, handle);
    assert!(backend.live.resource_watch_versions().is_empty());
    backend.live.open_fails.store(false, Ordering::SeqCst);
    // resource-watch.ts::RESOURCE_RECOVERY_INTERVAL.
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(15_000));
    cx.run_until_parked();
    assert!(!shows_rows(cx, handle), "the rows are still loading");
    let versions = backend.live.resource_watch_versions();
    assert!(!versions.is_empty());
    assert!(
        versions
            .iter()
            .all(|version| version.as_deref() == Some(START_VERSION)),
        "{versions:?}"
    );
    release.send(()).unwrap();
    cx.run_until_parked();
    assert!(shows_rows(cx, handle));
}

/// Opens the other context with start version 42 and its rows held, and makes
/// those rows list at version 50 when released.
fn load_at_list_version(
    cx: &mut TestAppContext,
    backend: &Controlled,
    handle: gpui_kit::AnyWindowHandle,
) -> tokio::sync::oneshot::Sender<()> {
    let release = load_other_context(cx, backend, handle);
    let signatures: Vec<_> = backend
        .live
        .streams
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(target, tx)| match target {
            WatchTarget::Resource { key, .. } if !tx.is_closed() => {
                Some(kubecove_gpui::backend::watch_signature(key))
            }
            _ => None,
        })
        .collect();
    assert!(!signatures.is_empty());
    backend.live.list_versions.lock().unwrap().extend(
        signatures
            .into_iter()
            .map(|signature| (signature, "50".into())),
    );
    release
}

fn start_versions_backend() -> Arc<Controlled> {
    Arc::new(Controlled {
        live: LiveControls {
            start_versions: Some(false),
            ..Default::default()
        },
        ..Default::default()
    })
}

// Failure mode: a refresh unsettles the rows again, and a watch that reopens
// during it goes back to the first load's start version, which can be hours
// old, instead of the version of the rows the view shows.
#[gpui_kit::test]
fn a_watch_reopened_during_a_refresh_uses_the_rows_version(cx: &mut TestAppContext) {
    let backend = start_versions_backend();
    let handle = open(cx, backend.clone());
    let release = load_at_list_version(cx, &backend, handle);
    release.send(()).unwrap();
    cx.run_until_parked();
    assert!(shows_rows(cx, handle));
    let (_hold, pending) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(pending);
    refresh(cx, handle);
    cx.run_until_parked();
    backend.live.send(StreamMessage::Stopped {
        stream_id: "native".into(),
    });
    cx.run_until_parked();
    // resource-watch.ts::RESOURCE_RECOVERY_INTERVAL.
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(15_000));
    cx.run_until_parked();
    let versions = backend.live.resource_watch_versions();
    assert!(!versions.is_empty());
    assert!(
        versions
            .iter()
            .all(|version| version.as_deref() == Some("50")),
        "{versions:?}"
    );
}

// Failure mode: the rows settle while a page that watches nothing is open, so
// nothing drops their start version. After a refresh there, a watching page
// opens its watches at that old version instead of waiting for the new rows.
#[gpui_kit::test]
fn rows_settled_off_a_watching_page_drop_their_start_version(cx: &mut TestAppContext) {
    let backend = start_versions_backend();
    let handle = open(cx, backend.clone());
    let release = load_at_list_version(cx, &backend, handle);
    let open_page = |cx: &mut TestAppContext, id: &'static str| {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    open_page(cx, "nav:GitOps");
    release.send(()).unwrap();
    cx.run_until_parked();
    let (_hold, pending) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(pending);
    refresh(cx, handle);
    cx.run_until_parked();
    open_page(cx, "nav:Workspace Overview");
    let versions = backend.live.resource_watch_versions();
    assert!(
        versions
            .iter()
            .all(|version| version.as_deref() != Some(START_VERSION)),
        "{versions:?}"
    );
}

#[gpui_kit::test]
fn a_failed_start_version_waits_for_the_rows(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled {
        live: LiveControls {
            start_versions: Some(true),
            ..Default::default()
        },
        ..Default::default()
    });
    let handle = open(cx, backend.clone());
    let release = load_other_context(cx, &backend, handle);
    assert!(!shows_rows(cx, handle), "the rows are still loading");
    assert!(backend.live.resource_watch_versions().is_empty());
    release.send(()).unwrap();
    cx.run_until_parked();
    assert!(shows_rows(cx, handle));
    assert_eq!(label(cx, handle).as_deref(), Some("Live"));
    assert!(
        backend
            .live
            .resource_watch_versions()
            .iter()
            .all(|version| version.as_deref() != Some(START_VERSION))
    );
}

#[gpui_kit::test]
fn deterministic_resource_read_errors_wait_for_manual_refresh(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.fail.store(true, Ordering::SeqCst);
    let handle = open(cx, backend.clone());
    backend.fail.store(false, Ordering::SeqCst);
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(15_000));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .try_find("resource:Deployment:payments:payments-api:mock-dev")
                .is_none()
        );
    })
    .unwrap();
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .try_find("resource:Deployment:payments:payments-api:mock-dev")
                .is_some()
        );
        assert_eq!(window.find("live-label").label(), Some("Live"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn initial_event_read_errors_recover_on_the_web_tick(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.live.events_fail.store(true, Ordering::SeqCst);
    let handle = open(cx, backend.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("resource:Deployment:payments:payments-api:mock-dev", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("show-events", cx);
        window.render_frame(cx);
        assert!(window.try_find("event:0").is_none());
    })
    .unwrap();
    backend.live.events_fail.store(false, Ordering::SeqCst);
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(15_000));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("event:0").label(),
            Some("BackOff Back-off restarting failed container app")
        );
    })
    .unwrap();
}
impl LiveControls {
    pub fn events(&self) -> Read<Vec<ResourceEventSummary>> {
        let fail = self.events_fail.load(Ordering::SeqCst);
        Box::pin(async move {
            if fail {
                return Err(AppError::new("Events unavailable", AppErrorKind::Network));
            }
            Ok(serde_json::from_value(json!([
                {"eventType": "Normal", "reason": "Pulled", "message": "Image present.", "count": 1, "lastSeen": "8m", "lastSeenAt": "2026-06-29T09:52:00Z", "source": "kubelet", "namespace": "payments"},
                {"eventType": "Warning", "reason": "BackOff", "message": "Back-off restarting failed container app", "count": 4, "lastSeen": "2m", "lastSeenAt": "2026-06-29T09:58:00Z", "source": "kubelet", "namespace": "payments"},
            ]))
            .unwrap())
        })
    }
    pub fn open(&self, target: WatchTarget) -> Read<LogStream> {
        if self.open_fails.load(Ordering::SeqCst) {
            return Box::pin(async { Err(AppError::new("watch refused", AppErrorKind::Network)) });
        }
        let (sender, messages) = mpsc::unbounded_channel();
        sender
            .send(StreamMessage::Status {
                stream_id: "native".into(),
                status: StreamStatus::Connected,
                message: "Live".into(),
            })
            .unwrap();
        self.streams.lock().unwrap().push((target, sender.clone()));
        let stream = LogStream::new(messages, ());
        let gate = self.gate.lock().unwrap().take();
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            Ok(stream)
        })
    }
    pub(super) fn send(&self, message: StreamMessage) {
        for (_, sender) in self.streams.lock().unwrap().iter() {
            if !sender.is_closed() {
                sender.send(message.clone()).unwrap();
            }
        }
    }
    pub(super) fn open_watches(&self, kind: &str) -> usize {
        self.streams
            .lock()
            .unwrap()
            .iter()
            .filter(|(target, tx)| {
                !tx.is_closed()
                    && matches!(target, WatchTarget::Resource { key, .. } if key.resource_kind.kind == kind)
            })
            .count()
    }
    fn active(&self) -> usize {
        self.streams
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, tx)| !tx.is_closed())
            .count()
    }
}

#[gpui_kit::test]
fn stream_errors_preserve_rows_and_connected_clears_stale(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    backend.live.send(StreamMessage::Error {
        stream_id: "native".into(),
        message: "Transport dropped".into(),
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("live-label").label(), Some("Reconnecting"));
        assert!(
            window
                .try_find("resource:Deployment:payments:payments-api:mock-dev")
                .is_some()
        );
    })
    .unwrap();
    backend.live.send(StreamMessage::Status {
        stream_id: "native".into(),
        status: StreamStatus::Connected,
        message: "Recovered".into(),
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("live-label").label(), Some("Live"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn streams_cannot_outlive_their_page_or_detail(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    assert!(backend.live.active() > 0);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("resource:Deployment:payments:payments-api:mock-dev", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let events = backend
        .live
        .streams
        .lock()
        .unwrap()
        .iter()
        .filter(|(target, tx)| matches!(target, WatchTarget::Events(_)) && !tx.is_closed())
        .map(|(_, tx)| tx.clone())
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("close-detail", cx);
        window.click("nav:Incidents", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(events[0].is_closed());
    assert_eq!(backend.live.active(), 0);
    backend.live.send(StreamMessage::Error {
        stream_id: "old".into(),
        message: "Late error".into(),
    });
}

#[gpui_kit::test]
fn changing_context_cancels_the_old_stream_receivers(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let old = backend
        .live
        .streams
        .lock()
        .unwrap()
        .iter()
        .map(|(_, tx)| tx.clone())
        .collect::<Vec<_>>();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(old.iter().all(mpsc::UnboundedSender::is_closed));
}

#[gpui_kit::test]
fn closed_channels_recover_at_the_existing_tick(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    // Close halfway through resource-watch.ts's existing recovery interval.
    let half = std::time::Duration::from_millis(15_000) / 2;
    cx.background_executor.advance_clock(half);
    backend.live.streams.lock().unwrap().clear();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("live-label").label(), Some("Reconnecting"));
    })
    .unwrap();
    cx.background_executor.advance_clock(half);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("live-label").label(), Some("Live"));
    })
    .unwrap();
}

// Failure mode: a named reload fails, the next notice names other rows, and
// its named reload succeeds. That clears the failure and stops recovery, so
// the rows the failed reload missed stay stale while the watch shows Live.
#[gpui_kit::test]
fn a_notice_after_a_failed_reload_lists_the_whole_kind(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let changed = |name: &str| StreamMessage::ResourceChanged {
        stream_id: "native".into(),
        target: WatchResourceTarget {
            cluster: "mock-dev".into(),
            kind: "Deployment".into(),
            namespace: Some("payments".into()),
            name: Some(name.into()),
        },
        action: "modified".into(),
    };
    // live_data.rs::COALESCE, the pause after each reload.
    let coalesce = std::time::Duration::from_millis(250);
    backend.fail.store(true, Ordering::SeqCst);
    backend.live.send(changed("payments-api"));
    cx.run_until_parked();
    cx.background_executor.advance_clock(coalesce);
    cx.run_until_parked();
    // The change the failed reload missed: the Deployment is gone.
    backend.fail.store(false, Ordering::SeqCst);
    backend.sessions.enabled.store(true, Ordering::SeqCst);
    backend.live.send(changed("other-api"));
    cx.run_until_parked();
    cx.background_executor.advance_clock(coalesce);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .try_find("resource:Deployment:payments:payments-api:mock-dev")
                .is_none()
        );
    })
    .unwrap();
}
