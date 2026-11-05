use super::*;
use kubecove_lib::models::{StreamMessage, WatchResourceTarget};

fn click(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_watch_refresh_supersedes_a_pending_kind_selection_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    click(cx, handle, "kinds-picker");
    click(cx, handle, "pick-kind:Deployment");
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receiver);
    click(cx, handle, "pick-kind:Deployment");
    assert!(backend.pending.lock().unwrap().is_none());
    backend.live.send(StreamMessage::ResourceChanged {
        stream_id: "native".into(),
        target: WatchResourceTarget {
            cluster: "mock-dev".into(),
            kind: "Deployment".into(),
            namespace: Some("payments".into()),
            name: None,
        },
        action: "applied".into(),
    });
    cx.run_until_parked();
    assert!(
        sender.send(()).is_err(),
        "an older selection read could overwrite the watch refresh"
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.find("resource:Deployment:payments:payments-api");
    })
    .unwrap();
}
