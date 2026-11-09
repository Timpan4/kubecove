use super::*;
use kubecove_gpui::backend::WatchTarget;
use kubecove_lib::models::{StreamMessage, StreamStatus};
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
    gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    streams: Mutex<Vec<(WatchTarget, mpsc::UnboundedSender<StreamMessage>)>>,
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
