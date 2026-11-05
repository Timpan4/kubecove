use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
pub(super) struct PageControls {
    pub inventory: Mutex<Option<GitOpsInventory>>,
    pub gate: Mutex<Option<(String, tokio::sync::oneshot::Receiver<()>)>>,
    pub fail: AtomicBool,
    pub reads: AtomicUsize,
}

#[gpui_kit::test]
fn switching_context_cancels_each_page_read(cx: &mut TestAppContext) {
    for page in ["Helm", "RBAC", "Incidents", "GitOps"] {
        let backend = Arc::new(Controlled::default());
        let handle = open(cx, backend.clone());
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(30_000));
        let (sender, receiver) = tokio::sync::oneshot::channel();
        *backend.pages.gate.lock().unwrap() = Some((page.into(), receiver));
        click(cx, handle, &format!("nav:{page}"));
        cx.update_window(handle, |_, window, cx| {
            window.within("context").click("input", cx);
            window.press("down", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(sender.send(()).is_err(), "{page} kept the old context read");
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("scope-context").label(), Some("mock-other"));
            window.remove_window();
        })
        .unwrap();
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn a_gitops_change_during_a_read_gets_a_trailing_refresh(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    *backend.pages.inventory.lock().unwrap() = Some(GitOpsInventory {
        argo: Some(vec![]),
        ..Default::default()
    });
    let handle = open(cx, backend.clone());
    click(cx, handle, "nav:GitOps");
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.pages.gate.lock().unwrap() = Some(("GitOps".into(), receiver));
    let change = kubecove_lib::models::StreamMessage::ResourceChanged {
        stream_id: "native".into(),
        target: kubecove_lib::models::WatchResourceTarget {
            cluster: "mock-dev".into(),
            kind: "Application".into(),
            namespace: Some("argocd".into()),
            name: None,
        },
        action: "applied".into(),
    };
    backend.live.send(change.clone());
    cx.run_until_parked();
    assert!(backend.pages.gate.lock().unwrap().is_none());
    *backend.pages.inventory.lock().unwrap() = Some(GitOpsInventory::default());
    backend.live.send(change);
    cx.run_until_parked();
    sender.send(()).unwrap();
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(250));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("gitops-unavailable").label(),
            Some("No GitOps providers detected")
        );
    })
    .unwrap();
    let reads = backend.pages.reads.load(Ordering::SeqCst);
    click(cx, handle, "namespace:payments");
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(15_000));
    cx.run_until_parked();
    assert_eq!(
        backend.pages.reads.load(Ordering::SeqCst),
        reads,
        "GitOps refresh survived leaving its page"
    );
}

impl PageControls {
    pub fn read<T: Send + 'static>(&self, page: &str, base: Read<T>) -> Read<T> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let mut gate = self.gate.lock().unwrap();
        let receiver = if gate.as_ref().is_some_and(|(target, _)| target == page) {
            gate.take().map(|(_, receiver)| receiver)
        } else {
            None
        };
        let failed = self.fail.load(Ordering::SeqCst);
        Box::pin(async move {
            if let Some(receiver) = receiver {
                let _ = receiver.await;
            }
            if failed {
                return Err(AppError::new("Page refresh refused", AppErrorKind::Network));
            }
            base.await
        })
    }
}

fn click(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, id: &str) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(gpui_kit::ElementId::Name(id.to_owned().into()), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn leaving_each_page_cancels_its_read_and_rejects_late_results(cx: &mut TestAppContext) {
    for page in ["Helm", "RBAC", "Incidents", "GitOps"] {
        let backend = Arc::new(Controlled::default());
        let handle = open(cx, backend.clone());
        // GitOps inventory was read for the overview. The destination query is stale.
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(30_000));
        let (sender, receiver) = tokio::sync::oneshot::channel();
        *backend.pages.gate.lock().unwrap() = Some((page.into(), receiver));
        click(cx, handle, &format!("nav:{page}"));
        assert!(
            backend.pages.gate.lock().unwrap().is_none(),
            "{page} must start its read"
        );
        click(cx, handle, "namespace:payments");
        // Cancellation drops the receiver. A reply cannot mutate the departed page.
        assert!(sender.send(()).is_err(), "{page} read survived navigation");
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(
                window
                    .try_find("resource:Deployment:payments:payments-api")
                    .is_some()
            );
            window.remove_window();
        })
        .unwrap();
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn a_failed_incident_refresh_preserves_rows_and_exposes_the_error(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    click(cx, handle, "nav:Incidents");
    backend.pages.fail.store(true, Ordering::SeqCst);
    click(cx, handle, "incidents-refresh");
    for millis in [1000, 2000] {
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(millis));
        cx.run_until_parked();
    }
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.try_find("incidents-refresh").is_some(),
            "stale incident controls must remain usable"
        );
        assert!(window.try_find("surface-error").is_some());
        assert_eq!(
            window.find("surface-error").description(),
            Some("Page refresh refused")
        );
    })
    .unwrap();
    backend.pages.fail.store(false, Ordering::SeqCst);
    click(cx, handle, "incidents-refresh");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("surface-error").is_none());
    })
    .unwrap();
}
