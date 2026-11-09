//! Failure modes: namespace replies cross context, cancellation keeps reads alive,
//! and failed pin writes lose the edit or announce false durability.
use super::*;
use kubecove_gpui::preferences::{Document, Store};

#[derive(Default)]
pub(super) struct ShellControls {
    pub pending: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    pub fail_save: AtomicBool,
    pub preferences: Store,
}

impl ShellControls {
    pub fn namespaces(&self) -> Read<Vec<NamespaceSummary>> {
        let pending = self.pending.lock().unwrap().take();
        Box::pin(async move {
            if let Some(pending) = pending {
                let _ = pending.await;
            }
            Ok(vec![])
        })
    }
    pub fn save(&self, document: Document) -> Result<Document, AppError> {
        if self.fail_save.load(Ordering::SeqCst) {
            Err(AppError::new(
                "Scripted preference write failure",
                AppErrorKind::Io,
            ))
        } else {
            self.preferences.save(document)
        }
    }
}

#[gpui_kit::test]
fn launcher_context_change_drops_the_old_namespace_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.shell.pending.lock().unwrap() = Some(receiver);
    click(cx, handle, "header-workspaces");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("workspace-context", cx);
        window.render_frame(cx);
        window.click("workspace-context:mock-other", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        sender.send(()).is_err(),
        "old namespace read survived context selection"
    );
}

#[gpui_kit::test]
fn failed_pin_save_keeps_the_edit_visible_and_the_store_unchanged(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    click(cx, handle, "nav:Workloads");
    backend.shell.fail_save.store(true, Ordering::SeqCst);
    click(cx, handle, "pin:Deployment:payments:payments-api:mock-dev");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window
                .find("pin:Deployment:payments:payments-api:mock-dev")
                .label(),
            Some("Unpin Deployment payments-api")
        );
        assert_eq!(
            window.find("notice").label(),
            Some("Scripted preference write failure")
        );
    })
    .unwrap();
    assert!(
        backend
            .shell
            .preferences
            .load()
            .unwrap()
            .workspace_entry_points
            .is_empty()
    );
    backend.shell.fail_save.store(false, Ordering::SeqCst);
    click(cx, handle, "pin:Deployment:payments:payments-api:mock-dev");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("notice").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn workspace_change_cancels_overview_retry_and_rejects_old_success(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.fail.store(true, Ordering::SeqCst);
    let handle = open(cx, backend.clone());
    click(cx, handle, "nav:Workspace Overview");
    backend.fail.store(false, Ordering::SeqCst);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receiver);
    click(cx, handle, "retry-overview");
    click(cx, handle, "header-workspaces");
    click(cx, handle, "workspace-create");
    assert!(
        sender.send(()).is_err(),
        "old retry survived workspace selection"
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("overview-metric:Total").label(),
            Some("Total: 1")
        );
        assert!(window.try_find("overview-retry-success").is_none());
        assert!(window.try_find("retry-overview").is_none());
    })
    .unwrap();
}

fn click(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, id: &str) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let offset = if id.starts_with("pin:") {
            let table = window.find("resource-table").bounds();
            let pin = window
                .find(gpui_kit::ElementId::Name(id.to_owned().into()))
                .bounds();
            Some(table.center().x - pin.center().x)
        } else {
            None
        };
        if let Some(offset) = offset {
            window.scroll(
                "resource-table",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(offset, px(0.))),
                cx,
            );
            window.render_frame(cx);
        }
        window.click(gpui_kit::ElementId::Name(id.to_owned().into()), cx);
        if let Some(offset) = offset {
            window.scroll(
                "resource-table",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(-offset, px(0.))),
                cx,
            );
        }
    })
    .unwrap();
    cx.run_until_parked();
}
