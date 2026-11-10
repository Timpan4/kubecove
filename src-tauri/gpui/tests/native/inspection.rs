//! Inspection flows through the controlled backend that the browser mocks
//! cannot express: a failed read and its retry, a cancelled read on a context
//! switch, a dirty draft, and what stays in the workspace scope.

use super::*;

#[gpui_kit::test]
fn a_read_failure_is_visible_and_can_be_retried(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.fail.store(true, Ordering::SeqCst);
    let handle = open(cx, backend.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("load-error").label(),
            Some("Failed to load resources")
        );
        assert!(window.try_find("empty").is_none());
    })
    .unwrap();
    backend.fail.store(false, Ordering::SeqCst);
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("load-error").is_none());
        assert!(
            window
                .find("resource:Deployment:payments:payments-api:mock-dev")
                .visible()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn kubeconfig_settings_the_backend_cannot_save_stay_disabled(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-settings", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-section:Kubeconfig", cx);
        window.render_frame(cx);
        let toggle = "setting:Show kubeconfig source labels";
        assert_eq!(window.find(toggle).description(), Some(KUBECONFIG_WRITES));
        window.click(toggle, cx);
        window.click("kubeconfig-env-save", cx);
        window.click("kubeconfig-env-reset", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let toggle = window.find("setting:Show kubeconfig source labels");
        assert_eq!(toggle.checked(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_ownership_map_opens_resources(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        // The first frame measures the graph viewport; the second fits it.
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find("map-counts").label(), Some("1 nodes / 0 edges"));
        // An ownerless resource starts inside its collapsed kind group.
        assert!(window.try_find(format!("map-node:{MAP_NODE}")).is_none());
        window.click("map-group:Deployment", cx);
        window.render_frame(cx);
        window.click(format!("map-node:{MAP_NODE}"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    // A one-context workspace has no context badge; the clicked node's ID
    // names the mock-dev object whose detail opens.
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("detail-identity").label(),
            Some("Deployment payments payments-api")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn switching_context_cancels_the_old_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receive);
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        send.send(()).is_err(),
        "old read must release its receiver on scope change"
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find("scope-context")
                .label()
                .unwrap()
                .contains("mock-other")
        );
        window.click("resource:Deployment:payments:payments-api:mock-other", cx);
    })
    .unwrap();
    cx.run_until_parked();
    // The clicked row's ID names the mock-other object; its detail opens.
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("detail-identity").label(),
            Some("Deployment payments payments-api")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_dirty_draft_requires_explicit_discard(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.click("resource:Deployment:payments:payments-api:mock-dev", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("show-yaml", cx);
        window.render_frame(cx);
        window.click("edit-draft", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        detail_tabs::edit_draft(window, cx, "kind: Deployment\nmetadata:\n  name: changed\n");
        window.click("close-detail", cx);
        window.render_frame(cx);
        assert!(
            window.find("close-detail").visible(),
            "a dirty draft blocks closing"
        );
        window.click("discard-draft", cx);
        window.render_frame(cx);
        window.click("close-detail", cx);
        assert!(window.try_find("detail-identity").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn warning_events_feed_the_incident_summary_and_timeline(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("resource:Deployment:payments:payments-api:mock-dev", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("incident-signal:Warning events").label(),
            Some("1 warning reason · 4 repeats")
        );
        // Only warnings reach the timeline; BackOff gets its presentation title.
        assert!(window.find("incident-timeline").visible());
    })
    .unwrap();
}

/// Failure mode: sessions from another context or kubeconfig source leak into
/// the page. The browser mocks hold sessions from one source only.
#[gpui_kit::test]
fn port_forwards_stay_in_the_workspace_scope(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        // Both the header and page exclude sessions from another source or context.
        assert_eq!(
            window.find("live-sessions").label(),
            Some("Open live sessions (1 active)")
        );
        window.click("live-sessions", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("live-sessions-manage", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("forward:visible").label(),
            Some("Pod/payments-api-0:8080")
        );
        assert!(window.try_find("forward:other-source").is_none());
        assert!(window.try_find("forward:other-context").is_none());
    })
    .unwrap();
}
