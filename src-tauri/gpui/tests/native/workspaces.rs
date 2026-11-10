use super::*;
use gpui_kit::test::TestQueryExt;

#[derive(Default)]
pub(super) struct WorkspaceControls {
    enabled: AtomicBool,
    fail_other: AtomicBool,
    pending: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    /// The file the next Import dialog returns.
    pub import: Mutex<Option<String>>,
}
impl WorkspaceControls {
    pub fn import(&self) -> Read<Option<String>> {
        let content = self.import.lock().unwrap().clone();
        Box::pin(async move { Ok(content) })
    }
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
                versions: Default::default(),
                listed: vec![],
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
/// Failure mode: Replace overwrites the first saved workspace whose display
/// name matches, although a later one shares the file's metadata name.
#[gpui_kit::test]
fn import_replace_prefers_the_shared_metadata_name(cx: &mut TestAppContext) {
    let shared = |name: &str| json!({"apiVersion":"kubecove.dev/workspace/v1","kind":"Workspace","metadata":{"name":"controlled"},"spec":{"displayName":name,"scope":{"clusterContext":"mock-dev","namespaces":["payments"],"kinds":["Pod"],"layout":"overview"},"shortcuts":[],"portForwards":[]}});
    let saved = |id: u64, name: &str| kubecove_gpui::workspaces::SavedWorkspace {
        id,
        name: name.into(),
        context: "mock-other".into(),
        group: vec![],
        group_name: None,
        namespaces: vec![],
        kinds: vec![],
        updated: id,
        reviews: vec![],
    };
    let backend = Arc::new(Controlled::default());
    {
        let mut document = backend.argo.document.lock().unwrap();
        document.workspaces = vec![saved(1, "Controlled import"), saved(2, "Renamed import")];
        document
            .workspace_documents
            .insert(2, shared("Renamed import"));
    }
    *backend.workspace.import.lock().unwrap() = Some(shared("Controlled import").to_string());
    let handle = open(cx, backend.clone());
    launcher(cx, handle);
    for id in [
        "settings-button:Import",
        "workspace-import:0:Replace",
        "settings-button:Apply import",
    ] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    let document = backend.argo.document.lock().unwrap();
    let context = |id: u64| {
        let workspace = document.workspaces.iter().find(|saved| saved.id == id);
        workspace.map(|saved| saved.context.clone())
    };
    assert_eq!(
        context(2).as_deref(),
        Some("mock-dev"),
        "shared key not replaced"
    );
    assert_eq!(
        context(1).as_deref(),
        Some("mock-other"),
        "name match overwritten"
    );
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
