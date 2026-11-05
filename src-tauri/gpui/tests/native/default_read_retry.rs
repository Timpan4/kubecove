use super::*;
use std::time::Duration;

fn tick(cx: &mut TestAppContext, millis: u64) {
    cx.background_executor
        .advance_clock(Duration::from_millis(millis));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn default_scope_retry_waits_then_stops_without_intermediate_error(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    backend
        .default_retry
        .errors
        .lock()
        .unwrap()
        .extend([AppErrorKind::Network; 3]);
    refresh(cx, handle);
    cx.run_until_parked();
    let first = backend.default_retry.calls.load(Ordering::SeqCst);
    tick(cx, 999);
    assert_eq!(backend.default_retry.calls.load(Ordering::SeqCst), first);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("live-label").label(), Some("Refreshing…"));
    })
    .unwrap();
    tick(cx, 1);
    assert_eq!(
        backend.default_retry.calls.load(Ordering::SeqCst),
        first + 1
    );
    tick(cx, 1999);
    assert_eq!(
        backend.default_retry.calls.load(Ordering::SeqCst),
        first + 1
    );
    tick(cx, 1);
    assert_eq!(
        backend.default_retry.calls.load(Ordering::SeqCst),
        first + 2
    );
    tick(cx, 2000);
    assert_eq!(
        backend.default_retry.calls.load(Ordering::SeqCst),
        first + 2
    );
}

#[gpui_kit::test]
fn replacing_scope_owner_drops_timer_and_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    backend
        .default_retry
        .errors
        .lock()
        .unwrap()
        .push_back(AppErrorKind::Network);
    refresh(cx, handle);
    cx.run_until_parked();
    refresh(cx, handle);
    cx.run_until_parked();
    let replacement = backend.default_retry.calls.load(Ordering::SeqCst);
    tick(cx, 1000);
    assert_eq!(
        backend.default_retry.calls.load(Ordering::SeqCst),
        replacement
    );
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receiver);
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    assert!(sender.send(()).is_err());
}

#[gpui_kit::test]
fn leaving_each_page_drops_default_retry_timer(cx: &mut TestAppContext) {
    for page in ["Helm", "RBAC", "Incidents", "GitOps"] {
        let backend = Arc::new(Controlled::default());
        let handle = open(cx, backend.clone());
        tick(cx, 30_000);
        backend.pages.fail.store(true, Ordering::SeqCst);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(format!("nav:{page}"), cx);
        })
        .unwrap();
        cx.run_until_parked();
        let calls = backend.pages.reads.load(Ordering::SeqCst);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("namespace:payments", cx);
        })
        .unwrap();
        cx.run_until_parked();
        tick(cx, 1000);
        assert_eq!(
            backend.pages.reads.load(Ordering::SeqCst),
            calls,
            "{page} kept its timer"
        );
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
}
