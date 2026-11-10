use super::*;

fn click(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// A newly checked kind's watch starts at the version of its own rows, so it
/// waits for their read. A watch opened earlier could deliver a change that the
/// older read then overwrites.
#[gpui_kit::test]
fn a_checked_kind_watch_waits_for_its_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    click(cx, handle, "kinds-picker");
    click(cx, handle, "pick-kind:Deployment");
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receiver);
    click(cx, handle, "pick-kind:Deployment");
    assert!(backend.pending.lock().unwrap().is_none());
    assert_eq!(
        backend.live.open_watches("Deployment"),
        0,
        "the watch started before the rows it shows"
    );
    sender.send(()).unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.find("resource:Deployment:payments:payments-api:mock-dev");
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(backend.live.open_watches("Deployment"), 1);
}

/// Enter on a sidebar row reached by Tab selects it, as a click does. A press
/// that only expanded or collapsed left keyboard users unable to pick a scope.
#[gpui_kit::test]
fn enter_on_a_tabbed_sidebar_row_selects_it(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend);
    click(cx, handle, "nav:Network");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("nav:Network").description(), Some("selected"));
        window.press("tab", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("nav:Network").description(),
            Some("unselected"),
            "Enter must select the row Tab focused"
        );
    })
    .unwrap();
}
