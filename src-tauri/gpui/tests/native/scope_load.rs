//! The resource scope load (`state-machines.md` #5). Only GPUI can reach a
//! scope refresh during a first load: its GitOps live menu runs the resource
//! scope refresh, while the web's GitOps menu refreshes GitOps reads.
use super::*;
use std::time::Duration;

#[gpui_kit::test]
fn a_refresh_that_fails_during_a_first_load_fails_the_load(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    // The first read for the other context stays pending.
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receive);
    cx.update_window(handle, |_, window, cx| {
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    backend.fail.store(true, Ordering::SeqCst);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav:GitOps", cx);
    })
    .unwrap();
    cx.run_until_parked();
    refresh(cx, handle);
    cx.run_until_parked();
    assert!(
        send.send(()).is_err(),
        "the refresh must replace the first read"
    );
    // Without a namespace the Cluster Overview shows the scope load without reading it again.
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("nav:Cluster Overview", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(1_000));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("load-error").label(),
            Some("Failed to load resources")
        );
        assert!(window.try_find("resources-loading").is_none());
    })
    .unwrap();
}
