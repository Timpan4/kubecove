use super::*;
use gpui_kit::AnyWindowHandle;
use std::time::Duration;

const FIRST_EVENT: &str = "BackOff Back-off restarting failed container app";

fn open_events(cx: &mut TestAppContext, backend: Arc<Controlled>) -> AnyWindowHandle {
    let handle = open(cx, backend);
    show_deployment_events(cx, handle);
    handle
}

fn show_deployment_events(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("resource:Deployment:payments:payments-api:mock-dev", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("show-events", cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn event_sender(backend: &Controlled) -> mpsc::UnboundedSender<StreamMessage> {
    let streams = backend.live.streams.lock().unwrap();
    let (_, sender) = streams
        .iter()
        .find(|(target, tx)| {
            matches!(target, WatchTarget::Events(resource)
                if resource.cluster == "mock-dev"
                    && resource.kind == "Deployment"
                    && resource.namespace.as_deref() == Some("payments")
                    && resource.name == "payments-api")
                && !tx.is_closed()
        })
        .expect("selected resource has an Events watch");
    sender.clone()
}

fn notify_events(cx: &mut TestAppContext, backend: &Controlled, notices: usize) {
    let sender = event_sender(backend);
    let event = StreamMessage::ResourceEventsChanged {
        stream_id: "native".into(),
        target: WatchResourceTarget {
            cluster: "mock-dev".into(),
            kind: "Deployment".into(),
            namespace: Some("payments".into()),
            name: Some("payments-api".into()),
        },
        action: "applied".into(),
    };
    for _ in 0..notices {
        sender.send(event.clone()).unwrap();
    }
    cx.run_until_parked();
}

fn advance(cx: &mut TestAppContext, ms: u64) {
    cx.background_executor
        .advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn initial_event_read_errors_recover_on_the_web_tick(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.live.events_fail.store(true, Ordering::SeqCst);
    let handle = open_events(cx, backend.clone());
    assert_eq!(backend.live.events_reads.load(Ordering::SeqCst), 1);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("event:0").is_none());
        assert!(window.try_find("events-refresh-warning").is_none());
        assert_eq!(
            window.find("events-error").label(),
            Some("Failed to load events: Events unavailable")
        );
    })
    .unwrap();
    backend.live.events_fail.store(false, Ordering::SeqCst);
    advance(cx, 15_000);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("event:0").label(), Some(FIRST_EVENT));
        assert!(window.try_find("events-error").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn successful_event_snapshot_survives_refresh_failure(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (release, gate) = tokio::sync::oneshot::channel();
    *backend.live.events_gate.lock().unwrap() = Some(gate);
    let handle = open_events(cx, backend.clone());

    backend.live.events_fail.store(true, Ordering::SeqCst);
    notify_events(cx, &backend, 1);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("event:0").is_none());
        assert!(window.try_find("events-error").is_some());
    })
    .unwrap();
    release.send(()).unwrap();
    cx.run_until_parked();

    // The first failure finished before the initial read. A subsequent
    // failure also keeps the now-visible snapshot.
    for after_existing_snapshot in [false, true] {
        if after_existing_snapshot {
            advance(cx, 250);
            let before_refresh = backend.live.events_reads.load(Ordering::SeqCst);
            notify_events(cx, &backend, 1);
            assert_eq!(
                backend.live.events_reads.load(Ordering::SeqCst),
                before_refresh + 1
            );
        }
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("event:0").label(), Some(FIRST_EVENT));
            assert_eq!(
                window.find("events-refresh-warning").label(),
                Some("Events refresh failed: Events unavailable")
            );
            assert!(window.try_find("events-watch-warning").is_none());
            assert!(window.try_find("events-error").is_none());
        })
        .unwrap();
    }

    backend.live.events_fail.store(false, Ordering::SeqCst);
    advance(cx, 250);
    let before_replay = backend.live.events_reads.load(Ordering::SeqCst);
    notify_events(cx, &backend, 2);
    advance(cx, 250);
    assert_eq!(
        backend.live.events_reads.load(Ordering::SeqCst),
        before_replay + 1,
        "queued notices share one refresh"
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("event:0").label(), Some(FIRST_EVENT));
        assert!(window.try_find("events-refresh-warning").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn connected_catches_up_when_no_events_are_replayed(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open_events(cx, backend.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("event:0").label(), Some(FIRST_EVENT));
    })
    .unwrap();

    let sender = event_sender(&backend);
    sender
        .send(StreamMessage::Status {
            stream_id: "native".into(),
            status: StreamStatus::Reconnecting,
            message: "Watch reconnecting".into(),
        })
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("events-watch-warning").is_none());
        assert_eq!(window.find("event:0").label(), Some(FIRST_EVENT));
    })
    .unwrap();
    backend.live.events_empty.store(true, Ordering::SeqCst);
    sender
        .send(StreamMessage::Status {
            stream_id: "native".into(),
            status: StreamStatus::Connected,
            message: "Watch connected".into(),
        })
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("event:0").is_none());
        assert!(window.find("empty:No events").visible());
    })
    .unwrap();
    assert_eq!(backend.live.events_reads.load(Ordering::SeqCst), 2);
}

#[gpui_kit::test]
fn a_refused_watch_start_still_refreshes_events(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.live.events_open_fails.store(true, Ordering::SeqCst);
    let handle = open_events(cx, backend.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("event:0").label(), Some(FIRST_EVENT));
        assert!(window.find("events-watch-warning").visible());
    })
    .unwrap();

    backend.live.events_empty.store(true, Ordering::SeqCst);
    advance(cx, 15_000);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("empty:No events").visible());
        assert!(window.find("events-watch-warning").visible());
    })
    .unwrap();
    assert_eq!(backend.live.events_reads.load(Ordering::SeqCst), 2);
}

#[gpui_kit::test]
fn a_pending_watch_start_still_refreshes_events(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let (release, opening) = tokio::sync::oneshot::channel();
    *backend.live.gate.lock().unwrap() = Some(opening);
    show_deployment_events(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("event:0").label(), Some(FIRST_EVENT));
    })
    .unwrap();

    backend.live.events_empty.store(true, Ordering::SeqCst);
    advance(cx, 15_000);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("empty:No events").visible());
    })
    .unwrap();
    release.send(()).unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn notice_during_a_pending_event_refresh_triggers_another_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    open_events(cx, backend.clone());
    let (release, gate) = tokio::sync::oneshot::channel();
    *backend.live.events_gate.lock().unwrap() = Some(gate);
    notify_events(cx, &backend, 1);
    assert_eq!(backend.live.events_reads.load(Ordering::SeqCst), 2);
    notify_events(cx, &backend, 1);
    assert_eq!(backend.live.events_reads.load(Ordering::SeqCst), 2);

    release.send(()).unwrap();
    cx.run_until_parked();
    advance(cx, 250);
    assert_eq!(backend.live.events_reads.load(Ordering::SeqCst), 3);
}

#[gpui_kit::test]
fn a_stale_initial_failure_does_not_replace_a_successful_watch_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (release, gate) = tokio::sync::oneshot::channel();
    *backend.live.events_gate.lock().unwrap() = Some(gate);
    backend.live.events_fail.store(true, Ordering::SeqCst);
    let handle = open_events(cx, backend.clone());

    backend.live.events_fail.store(false, Ordering::SeqCst);
    notify_events(cx, &backend, 1);
    assert_eq!(backend.live.events_reads.load(Ordering::SeqCst), 2);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("event:0").label(), Some(FIRST_EVENT));
    })
    .unwrap();
    release.send(()).unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("event:0").label(), Some(FIRST_EVENT));
        assert!(window.try_find("events-error").is_none());
        assert!(window.try_find("events-refresh-warning").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_stale_initial_snapshot_does_not_replace_a_newer_empty_snapshot(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (release, gate) = tokio::sync::oneshot::channel();
    *backend.live.events_gate.lock().unwrap() = Some(gate);
    let handle = open_events(cx, backend.clone());
    backend.live.events_empty.store(true, Ordering::SeqCst);
    notify_events(cx, &backend, 1);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("empty:No events").visible());
    })
    .unwrap();
    release.send(()).unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("empty:No events").visible());
        assert!(window.try_find("event:0").is_none());
    })
    .unwrap();
}
