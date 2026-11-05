use super::*;
use std::{collections::VecDeque, sync::atomic::AtomicUsize, time::Duration};

#[derive(Default)]
pub(super) struct RetryControls {
    pub calls: AtomicUsize,
    pub errors: Mutex<VecDeque<AppErrorKind>>,
    pub gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

fn tick(cx: &mut TestAppContext, millis: u64) {
    cx.background_executor
        .advance_clock(Duration::from_millis(millis));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn cancelled_topology_uses_two_delays_then_exposes_failure(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    backend
        .retry
        .errors
        .lock()
        .unwrap()
        .extend([AppErrorKind::Cancelled; 3]);
    refresh(cx, handle);
    cx.run_until_parked();
    let initial = backend.retry.calls.load(Ordering::SeqCst);
    tick(cx, 999);
    assert_eq!(backend.retry.calls.load(Ordering::SeqCst), initial);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("map-error").is_none());
    })
    .unwrap();
    tick(cx, 1);
    assert_eq!(backend.retry.calls.load(Ordering::SeqCst), initial + 1);
    tick(cx, 1999);
    assert_eq!(backend.retry.calls.load(Ordering::SeqCst), initial + 1);
    tick(cx, 1);
    assert_eq!(backend.retry.calls.load(Ordering::SeqCst), initial + 2);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("map-error").is_some());
    })
    .unwrap();
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("map-error").is_none());
        assert!(window.try_find("map-counts").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn navigation_drops_the_topology_retry_timer(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    backend
        .retry
        .errors
        .lock()
        .unwrap()
        .push_back(AppErrorKind::Cancelled);
    refresh(cx, handle);
    cx.run_until_parked();
    let calls = backend.retry.calls.load(Ordering::SeqCst);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav:Incidents", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let after_navigation = backend.retry.calls.load(Ordering::SeqCst);
    tick(cx, 1000);
    assert_eq!(backend.retry.calls.load(Ordering::SeqCst), after_navigation);
    assert!(after_navigation >= calls);
}

#[gpui_kit::test]
fn a_new_scope_drops_the_old_topology_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.retry.gate.lock().unwrap() = Some(receiver);
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        sender.send(()).is_err(),
        "the old scope no longer owns a read"
    );
}

#[gpui_kit::test]
fn other_topology_errors_wait_for_manual_refresh(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    backend
        .retry
        .errors
        .lock()
        .unwrap()
        .push_back(AppErrorKind::Network);
    refresh(cx, handle);
    cx.run_until_parked();
    let calls = backend.retry.calls.load(Ordering::SeqCst);
    tick(cx, 3000);
    assert_eq!(backend.retry.calls.load(Ordering::SeqCst), calls);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("map-error").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn leaving_the_incident_guide_drops_its_topology_retry(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let before = backend.retry.calls.load(Ordering::SeqCst);
    backend
        .retry
        .errors
        .lock()
        .unwrap()
        .push_back(AppErrorKind::Cancelled);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav:Incidents", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let calls = backend.retry.calls.load(Ordering::SeqCst);
    assert_eq!(
        calls,
        before + 1,
        "the Pod guide owns a pending topology retry"
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav:RBAC", cx);
    })
    .unwrap();
    cx.run_until_parked();
    tick(cx, 1000);
    assert_eq!(backend.retry.calls.load(Ordering::SeqCst), calls);
}
