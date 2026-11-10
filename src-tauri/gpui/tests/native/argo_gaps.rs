//! Forget's two stores can fail separately. Retrying must preserve exact identity,
//! disable automatic reconnect after partial cleanup, and obtain a new review.
use super::argo::{connect, connect_remembered, open_argo, scroll, step};
use super::*;

fn forget(cx: &mut TestAppContext, h: gpui_kit::AnyWindowHandle, id: &str) {
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-connect-open", cx);
        w.click("argo-connect-open", cx);
        w.render_frame(cx);
        let id = format!("argo-forget:{id}");
        scroll::reveal(w, "detail-body", &id, cx);
        w.click(id, cx);
    });
}

#[gpui_kit::test]
fn forget_failures_preserve_retryable_profile_without_auto_reconnect(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let unrelated = kubecove_gpui::preferences::ArgoProfile {
        id: "other-profile".into(),
        endpoint: kubecove_gpui::preferences::Endpoint::ExternalHttps {
            url: "https://other.example.test".into(),
        },
        cluster_context: Some("other-context".into()),
        workspace_id: Some("other-workspace".into()),
        kubeconfig_source_key: None,
        remember_credential: true,
    };
    backend
        .argo
        .document
        .lock()
        .unwrap()
        .preferences
        .argo_profiles
        .push(unrelated.clone());
    let h = open_argo(cx, backend.clone());
    connect_remembered(cx, h);
    let id = backend
        .argo
        .document
        .lock()
        .unwrap()
        .preferences
        .argo_profiles
        .iter()
        .find(|p| p.id != unrelated.id)
        .unwrap()
        .id
        .clone();
    backend.argo.forget_fail.store(true, Ordering::SeqCst);
    forget(cx, h, &id);
    step(cx, h, |w, _| {
        assert!(
            w.find("argo-connection-error")
                .label()
                .unwrap()
                .contains("credential deletion failed")
        )
    });
    assert!(
        !backend
            .argo
            .document
            .lock()
            .unwrap()
            .preferences
            .argo_profiles
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .remember_credential
    );
    assert!(backend.argo.forgotten.lock().unwrap().is_empty());
    backend.argo.forget_fail.store(false, Ordering::SeqCst);
    backend.argo.fail_save.store(
        backend.argo.saves.load(Ordering::SeqCst) + 2,
        Ordering::SeqCst,
    );
    forget(cx, h, &id);
    step(cx, h, |w, _| {
        assert!(
            w.find("argo-connection-error")
                .label()
                .unwrap()
                .contains("preferences write failed")
        )
    });
    assert_eq!(*backend.argo.forgotten.lock().unwrap(), vec![id.clone()]);
    assert!(
        !backend
            .argo
            .document
            .lock()
            .unwrap()
            .preferences
            .argo_profiles
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .remember_credential
    );
    backend.argo.fail_save.store(0, Ordering::SeqCst);
    forget(cx, h, &id);
    assert_eq!(
        backend
            .argo
            .document
            .lock()
            .unwrap()
            .preferences
            .argo_profiles,
        vec![unrelated]
    );
}

#[gpui_kit::test]
fn forget_does_not_delete_credential_when_reconnect_opt_out_cannot_save(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let h = open_argo(cx, backend.clone());
    // A remembered profile, so forget must first save the reconnect opt-out.
    connect_remembered(cx, h);
    let id = backend
        .argo
        .document
        .lock()
        .unwrap()
        .preferences
        .argo_profiles[0]
        .id
        .clone();
    backend.argo.fail_save.store(
        backend.argo.saves.load(Ordering::SeqCst) + 1,
        Ordering::SeqCst,
    );
    forget(cx, h, &id);
    assert!(backend.argo.forgotten.lock().unwrap().is_empty());
    let profile = backend
        .argo
        .document
        .lock()
        .unwrap()
        .preferences
        .argo_profiles[0]
        .clone();
    assert_eq!(profile.id, id);
    assert!(
        profile.remember_credential,
        "a failed opt-out save keeps automatic reconnect on"
    );
    step(cx, h, |w, _| {
        assert!(
            w.find("argo-connection-error")
                .label()
                .unwrap()
                .contains("before credential deletion")
        )
    });
}

#[gpui_kit::test]
fn retry_discovery_and_preview_recover_without_sending_credentials(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.argo.discovery_fail.store(true, Ordering::SeqCst);
    let h = open_argo(cx, backend.clone());
    step(cx, h, |w, cx| w.click("argo-connect-open", cx));
    for millis in [1000, 2000] {
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(millis));
        cx.run_until_parked();
    }
    backend.argo.discovery_fail.store(false, Ordering::SeqCst);
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-discovery-retry", cx);
        w.click("argo-discovery-retry", cx);
    });
    step(cx, h, |w, _| {
        assert!(w.try_find("argo-server:fixture-service").is_some())
    });
    assert_eq!(backend.argo.connections.load(Ordering::SeqCst), 0);
}

#[gpui_kit::test]
fn hard_and_normal_refresh_keep_distinct_review_requests(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let h = open_argo(cx, backend.clone());
    for label in ["Hard refresh", "Refresh"] {
        if label == "Hard refresh" {
            step(cx, h, |w, cx| {
                scroll::reveal(w, "detail-body", "argo-refresh-options", cx);
                w.click("argo-refresh-options", cx);
            });
        }
        step(cx, h, |w, cx| {
            let id = format!("argo-operation:{label}");
            scroll::reveal(w, "detail-body", &id, cx);
            w.click(id, cx);
        });
        step(cx, h, |w, cx| {
            scroll::reveal(w, "detail-body", "argo-review-cancel", cx);
            w.click("argo-review-cancel", cx);
        });
    }
    let reviews = backend.argo.reviews.lock().unwrap();
    assert_eq!(
        reviews
            .iter()
            .map(|r| r.action.as_str())
            .collect::<Vec<_>>(),
        ["hardRefresh", "refresh"]
    );
    assert!(
        reviews
            .iter()
            .all(|r| r.application.uid.as_deref() == Some("original"))
    );
}

#[gpui_kit::test]
fn lazy_comparison_cancels_old_selection_and_closed_detail(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.argo.lazy.store(true, Ordering::SeqCst);
    let h = open_argo(cx, backend.clone());
    connect(cx, h);
    let (send, gate) = tokio::sync::oneshot::channel();
    *backend.argo.comparison_gate.lock().unwrap() = Some(gate);
    step(cx, h, |w, cx| {
        let id = "argo-change:apps:deployment:payments:same-name";
        scroll::reveal(w, "detail-body", id, cx);
        w.click(id, cx);
    });
    step(cx, h, |w, cx| {
        let id = "argo-change::configmap:payments:same-name";
        scroll::reveal(w, "detail-body", id, cx);
        w.click(id, cx);
    });
    assert!(
        send.send(()).is_err(),
        "the previous resource comparison must be dropped"
    );
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-view:Desired", cx);
        w.click("argo-view:Desired", cx);
    });
    step(cx, h, |w, _| {
        assert!(w.find("argo-yaml:1").label().unwrap().contains("ConfigMap"))
    });
    let (send, gate) = tokio::sync::oneshot::channel();
    *backend.argo.comparison_gate.lock().unwrap() = Some(gate);
    step(cx, h, |w, cx| {
        let id = "argo-change:apps:deployment:payments:same-name";
        scroll::reveal(w, "detail-body", id, cx);
        w.click(id, cx);
    });
    step(cx, h, |w, cx| w.click("close-detail", cx));
    assert!(
        send.send(()).is_err(),
        "closed detail must cancel its comparison"
    );
}

#[gpui_kit::test]
fn lazy_comparison_refusal_is_visible_and_retryable(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.argo.lazy.store(true, Ordering::SeqCst);
    backend.argo.comparison_fail.store(true, Ordering::SeqCst);
    let h = open_argo(cx, backend.clone());
    connect(cx, h);
    step(cx, h, |w, cx| {
        let id = "argo-change:apps:deployment:payments:same-name";
        scroll::reveal(w, "detail-body", id, cx);
        w.click(id, cx);
    });
    step(cx, h, |w, _| {
        assert!(w.try_find("argo-comparison-retry").is_some())
    });
    backend.argo.comparison_fail.store(false, Ordering::SeqCst);
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-comparison-retry", cx);
        w.click("argo-comparison-retry", cx);
    });
    step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-view:Desired", cx);
        w.click("argo-view:Desired", cx);
    });
    step(cx, h, |w, _| {
        assert!(
            w.find("argo-yaml:0")
                .label()
                .unwrap()
                .contains("Deployment")
        )
    });
}
