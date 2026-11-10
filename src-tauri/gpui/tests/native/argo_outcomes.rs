//! Failure modes: accepted means submitted, a failed read never means completion,
//! and replacing an Application must not credit the old operation to the new one.
use super::argo::scroll;

// A revision can occur in several deployments. Rollback must use the chosen ID.
#[gpui_kit::test]
fn rollback_keeps_the_selected_deployment_id(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.argo.repeated_revision.store(true, Ordering::SeqCst);
    let h = argo::open_argo(cx, backend);
    argo::connect(cx, h);
    argo::step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-history-picker", cx);
        w.click("argo-history-picker", cx);
        w.press("down", cx);
    });
    argo::step(cx, h, |w, cx| w.press("enter", cx));
    argo::step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-operation:Rollback", cx);
        w.click("argo-operation:Rollback", cx);
    });
    argo::step(cx, h, |w, _| {
        assert!(
            w.find("argo-review-options")
                .label()
                .unwrap()
                .contains("History: 17")
        );
    });
}
use super::*;

fn click_sync(cx: &mut TestAppContext, h: gpui_kit::AnyWindowHandle) {
    argo::step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-operation:Sync", cx);
        w.click("argo-operation:Sync", cx);
    });
}
// Failure mode: an operation started outside this client is not in its outcomes,
// so a review that trusts only those would patch a second sync over it.
#[gpui_kit::test]
fn review_refuses_an_operation_already_in_progress(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let h = argo::open_argo(cx, backend.clone());
    backend.argo.observation.store(4, Ordering::SeqCst);
    click_sync(cx, h);
    argo::step(cx, h, |w, _| {
        assert!(w.try_find("argo-review-confirm").is_none());
        assert!(
            w.find("argo-operation-result")
                .label()
                .unwrap()
                .contains("already has an operation in progress")
        );
    });
    assert!(backend.argo.reviews.lock().unwrap().is_empty());
}
// Failure mode: preflight reviews the tab's old resourceVersion after Argo CD wrote
// the Application, so every submission and retry fails validation.
#[gpui_kit::test]
fn review_uses_the_fresh_resource_version(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let h = argo::open_argo(cx, backend.clone());
    backend.argo.observation.store(6, Ordering::SeqCst);
    click_sync(cx, h);
    let reviews = backend.argo.reviews.lock().unwrap();
    let reviewed = reviews.last().expect("preflight ran");
    assert_eq!(reviewed.resource_version.as_deref(), Some("fresh"));
    assert_eq!(
        reviewed.application.resource_version.as_deref(),
        Some("fresh")
    );
}

fn submit(cx: &mut TestAppContext, backend: Arc<Controlled>) -> gpui_kit::AnyWindowHandle {
    backend.argo.accept.store(true, Ordering::SeqCst);
    let h = argo::open_argo(cx, backend.clone());
    argo::step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-operation:Sync", cx);
        w.click("argo-operation:Sync", cx);
    });
    argo::step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-review-confirm", cx);
        w.click("argo-review-confirm", cx);
    });
    assert_eq!(backend.argo.submitted.load(Ordering::SeqCst), 1);
    h
}
fn observe(cx: &mut TestAppContext, backend: Arc<Controlled>, outcome: usize, expected: &str) {
    let h = submit(cx, backend.clone());
    backend.argo.observation.store(outcome, Ordering::SeqCst);
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(15_000));
    cx.run_until_parked();
    argo::step(cx, h, |w, _| {
        assert!(w.find("argo-outcome").label().unwrap().contains(expected));
    });
}
#[gpui_kit::test]
fn accepted_failure_is_observed(cx: &mut TestAppContext) {
    observe(
        cx,
        Arc::new(Controlled::default()),
        1,
        "Argo CD reported failure",
    );
}
#[gpui_kit::test]
fn replacement_does_not_claim_completion(cx: &mut TestAppContext) {
    observe(
        cx,
        Arc::new(Controlled::default()),
        2,
        "Application was replaced",
    );
}
#[gpui_kit::test]
fn observation_errors_remain_pending(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let h = submit(cx, backend.clone());
    backend.argo.observation.store(4, Ordering::SeqCst);
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(15_000));
    cx.run_until_parked();
    backend.argo.observation.store(3, Ordering::SeqCst);
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(15_000));
    cx.run_until_parked();
    argo::step(cx, h, |w, cx| {
        assert!(
            w.find("argo-outcome")
                .label()
                .unwrap()
                .contains("Retrying observation")
        );
        scroll::reveal(w, "detail-body", "argo-operation:Sync", cx);
        w.click("argo-operation:Sync", cx);
    });
    assert_eq!(backend.argo.submitted.load(Ordering::SeqCst), 1);
    backend.argo.observation.store(5, Ordering::SeqCst);
    argo::step(cx, h, |w, cx| {
        scroll::reveal(w, "detail-body", "argo-state-retry", cx);
        w.click("argo-state-retry", cx);
    });
    argo::step(cx, h, |w, _| {
        assert!(
            w.find("argo-outcome")
                .label()
                .unwrap()
                .contains("Argo CD reported failure")
        )
    });
    assert_eq!(
        backend.argo.submitted.load(Ordering::SeqCst),
        1,
        "state refresh must not resubmit"
    );
}
