//! The header context picker after a saved kubeconfig source change. Only GPUI can show a
//! stale list here: the web keys its context query by the source key, so a read for the
//! previous source never reaches the new picker.
use super::*;

const ADDED_PATH: &str = "/fixtures/mock-added.kubeconfig";
const ADDED_CONTEXT: &str = "mock-added";

#[derive(Default)]
pub(super) struct KubeconfigControls {
    /// Lets the settings page save source changes.
    pub writable: AtomicBool,
    added: AtomicBool,
    /// Holds the next context read until the test releases it.
    pub contexts_pending: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    pub fail_contexts: AtomicBool,
}

impl KubeconfigControls {
    pub fn writes_unavailable(&self) -> Option<&'static str> {
        (!self.writable.load(Ordering::SeqCst)).then_some(KUBECONFIG_WRITES)
    }

    /// The added file changes the source key, as the backend's key hashes the added paths.
    pub fn sources(&self, read: Read<KubeconfigSourcesSummary>) -> Read<KubeconfigSourcesSummary> {
        let added = self.added.load(Ordering::SeqCst);
        Box::pin(async move {
            let mut sources = read.await?;
            if added {
                sources.source_key = format!("{}|{ADDED_CONTEXT}", sources.source_key);
                sources.paths = serde_json::from_value(json!([{ "path": ADDED_PATH }])).unwrap();
            }
            Ok(sources)
        })
    }

    pub fn add(&self, read: Read<KubeconfigSourcesSummary>) -> Read<KubeconfigSourcesSummary> {
        self.added.store(true, Ordering::SeqCst);
        self.sources(read)
    }

    /// The contexts of the source at the time of the call.
    pub fn contexts(&self, mut contexts: Vec<ClusterContext>) -> Read<Vec<ClusterContext>> {
        if self.added.load(Ordering::SeqCst) {
            contexts.push(ClusterContext {
                name: ADDED_CONTEXT.into(),
                is_current: false,
            });
        }
        let fail = self.fail_contexts.load(Ordering::SeqCst);
        let pending = self.contexts_pending.lock().unwrap().take();
        Box::pin(async move {
            if let Some(pending) = pending {
                let _ = pending.await;
            }
            if fail {
                return Err(AppError::new(
                    "Scripted contexts read failure",
                    // Not retried, as a malformed kubeconfig file is not.
                    AppErrorKind::Kubeconfig,
                ));
            }
            Ok(contexts)
        })
    }
}

/// Saves the fixture kubeconfig file as an added path, then returns to the workspace.
fn add_source(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    // Each click waits for the reads it starts, such as the Kubeconfig section's source read.
    for id in [
        "open-settings",
        "settings-section:Kubeconfig",
        "kubeconfig-paths-input",
    ] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    cx.update_window(handle, |_, window, cx| {
        window.input(ADDED_PATH, cx);
        window.render_frame(cx);
        window.click("settings-button:Add pasted paths", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .try_find(format!("kubeconfig-path:{ADDED_PATH}"))
                .is_some(),
            "the source change was not saved"
        );
        window.click("settings-back", cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// Searches the header picker for `context`, confirms the first match, and returns the
/// context the workspace then shows. A context the picker does not list changes nothing.
fn choose(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, context: &str) -> String {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.within("context").click("input", cx);
        window.render_frame(cx);
        window.input(context, cx);
        window.render_frame(cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window
            .find("scope-context")
            .label()
            .unwrap_or_default()
            .to_owned()
    })
    .unwrap()
}

/// Failure mode: the startup context read finishes after a saved source change and puts the
/// previous source's contexts back in the picker.
#[gpui_kit::test]
fn a_context_read_older_than_a_saved_source_does_not_fill_the_picker(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.kubeconfig.writable.store(true, Ordering::SeqCst);
    let (release, pending) = tokio::sync::oneshot::channel();
    *backend.kubeconfig.contexts_pending.lock().unwrap() = Some(pending);
    let handle = open(cx, backend.clone());
    add_source(cx, handle);
    let _ = release.send(());
    cx.run_until_parked();
    assert_eq!(
        choose(cx, handle, ADDED_CONTEXT),
        ADDED_CONTEXT,
        "the picker lost the saved source's context"
    );
}

/// Failure mode: the new source's context read fails and the picker keeps offering the
/// previous source's contexts.
#[gpui_kit::test]
fn a_failed_context_read_after_a_source_change_clears_the_picker(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.kubeconfig.writable.store(true, Ordering::SeqCst);
    let handle = open(cx, backend.clone());
    backend
        .kubeconfig
        .fail_contexts
        .store(true, Ordering::SeqCst);
    add_source(cx, handle);
    assert_eq!(
        choose(cx, handle, "mock-other"),
        "mock-dev",
        "the picker still offers the previous source's contexts"
    );
}
