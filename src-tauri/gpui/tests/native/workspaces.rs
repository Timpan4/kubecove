use super::*;
use gpui_kit::test::TestQueryExt;

#[derive(Default)]
pub(super) struct WorkspaceControls {
    enabled: AtomicBool,
    fail_other: AtomicBool,
    pending: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}
impl WorkspaceControls {
    pub fn read(&self, scope: &Scope) -> Option<Read<ScopeResult>> {
        if !self.enabled.load(Ordering::SeqCst) {
            return None;
        }
        let context = scope.context.clone();
        let fail = context == "mock-other" && self.fail_other.load(Ordering::SeqCst);
        let pending = if context == "mock-other" {
            self.pending.lock().unwrap().take()
        } else {
            None
        };
        Some(Box::pin(async move {
            if let Some(pending) = pending {
                let _ = pending.await;
            }
            if fail {
                return Err(AppError::new("Context unavailable", AppErrorKind::Cluster));
            }
            Ok(ScopeResult {
                resources: vec![resource(&context)],
                namespaces: vec![],
                warnings: vec![],
            })
        }))
    }
}
fn launcher(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("header-workspaces", cx);
    })
    .unwrap();
    cx.run_until_parked();
}
fn create_group(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    launcher(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("workspace-group:mock-other", cx);
        window.click("workspace-create", cx);
    })
    .unwrap();
    cx.run_until_parked();
}
#[gpui_kit::test]
fn one_failed_context_keeps_successful_rows_and_names_the_failure(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.workspace.enabled.store(true, Ordering::SeqCst);
    backend.workspace.fail_other.store(true, Ordering::SeqCst);
    let handle = open(cx, backend);
    create_group(cx, handle);
    for millis in [1000, 2000] {
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(millis));
        cx.run_until_parked();
    }
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("overview-metric:Total").label(),
            Some("Total: 1")
        );
        assert!(
            window
                .try_find_by_label("mock-other: Context unavailable")
                .is_some()
        );
    })
    .unwrap();
}
#[gpui_kit::test]
fn changing_workspace_cancels_every_old_context_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.workspace.enabled.store(true, Ordering::SeqCst);
    let handle = open(cx, backend.clone());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.workspace.pending.lock().unwrap() = Some(receiver);
    create_group(cx, handle);
    launcher(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("workspace-create", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(sender.send(()).is_err(), "old context read was dropped");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("overview-metric:Total").label(),
            Some("Total: 1")
        );
    })
    .unwrap();
}
