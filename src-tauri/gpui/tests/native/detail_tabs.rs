//! Native-only failure modes of the Logs, Actions and YAML apply tabs that the
//! browser mocks cannot express: stream and dry-run refusals, a failed
//! execution, a stream outliving its detail, and a draft edited after review.

use super::{Controlled, open};
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{AppContext, TestAppContext};
use kubecove_gpui::backend::{LogStream, LogTarget, Read};
use kubecove_lib::models::{
    AppError, AppErrorKind, ClusterOperationResult, DeploymentRevision, ResourceActionPreview,
    ResourceActionRequest, ResourceActionState, ResourceActionStateRequest, ResourceSummary,
    StreamMessage, YamlApplyPreview, YamlApplyRequest, YamlApplyResult, YamlViewMode,
};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Switches that make each detail-tab read fail, and counters for the
/// guarded operations that reached the backend.
#[derive(Default)]
pub struct DetailControls {
    log_fail: AtomicBool,
    log_stopped: Arc<AtomicBool>,
    preview_fail: AtomicBool,
    execute_fail: AtomicBool,
    executed: AtomicUsize,
    dry_run_fail: AtomicBool,
    /// Holds the next dry run until the test releases it.
    dry_run_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    applied: AtomicUsize,
    /// Holds the next apply-clean draft read until the test releases it.
    draft_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    /// Holds the next resource details read until the test releases it.
    pub details_gate: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    /// Scripted revision reads in order; an empty queue reads no revisions.
    revisions: Mutex<VecDeque<Result<Vec<DeploymentRevision>, AppError>>>,
    /// Reads of the read-only YAML view, as opposed to apply-clean drafts.
    view_reads: AtomicUsize,
}

/// Records that the backend stream stopped, as `StopStream` does natively.
struct StopFlag(Arc<AtomicBool>);

impl Drop for StopFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl DetailControls {
    pub fn deployment_revisions(&self) -> Read<Vec<DeploymentRevision>> {
        let next = self.revisions.lock().unwrap().pop_front();
        Box::pin(async move { next.unwrap_or_else(|| Ok(vec![])) })
    }

    pub fn log_stream(&self, _: LogTarget) -> Read<LogStream> {
        let fail = self.log_fail.load(Ordering::SeqCst);
        let stopped = self.log_stopped.clone();
        Box::pin(async move {
            if fail {
                return Err(AppError::new(
                    "pods/log is forbidden",
                    AppErrorKind::Forbidden,
                ));
            }
            let (sender, messages) = tokio::sync::mpsc::unbounded_channel();
            let _ = sender.send(StreamMessage::LogLine {
                stream_id: "logs".into(),
                line: "boot payments-api".into(),
                source: None,
            });
            Ok(LogStream::new(messages, (sender, StopFlag(stopped))))
        })
    }

    pub fn action_state(&self, request: ResourceActionStateRequest) -> Read<ResourceActionState> {
        let permission = |action: &str, verb: &str| json!({"action": action, "allowed": true, "reason": "", "requiredPermission": format!("{verb} deployments")});
        let state = json!({
            "target": request.target, "uid": "uid-1", "resourceVersion": "1", "replicas": 2,
            "paused": false, "containers": [], "labels": {}, "annotations": {},
            "permissions": [permission("edit", "patch"), permission("restart", "patch"), permission("delete", "delete")],
            "revisions": [],
        });
        Box::pin(async move { Ok(serde_json::from_value(state).unwrap()) })
    }

    pub fn preview_action(&self, request: ResourceActionRequest) -> Read<ResourceActionPreview> {
        let fail = self.preview_fail.load(Ordering::SeqCst);
        Box::pin(async move {
            if fail {
                return Err(AppError::new(
                    "dry run denied by admission webhook",
                    AppErrorKind::AdmissionDenied,
                ));
            }
            Ok(ResourceActionPreview {
                target: request.target,
                effect: "Restart".into(),
                before_yaml: "metadata:\n  name: payments-api\n".into(),
                after_yaml: "metadata:\n  name: payments-api\n  restartedAt: now\n".into(),
                uid: "uid-1".into(),
                resource_version: "1".into(),
                revision_uid: None,
                revision_resource_version: None,
            })
        })
    }

    pub fn execute_action(&self, request: ResourceActionRequest) -> Read<ClusterOperationResult> {
        let fail = self.execute_fail.load(Ordering::SeqCst);
        self.executed.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if fail {
                return Err(AppError::new(
                    "resourceVersion changed",
                    AppErrorKind::FieldManagerConflict,
                ));
            }
            Ok(ClusterOperationResult {
                target: request.target,
                effect: "Restarted".into(),
            })
        })
    }

    pub fn resource_yaml(&self, resource: ResourceSummary, mode: YamlViewMode) -> Read<String> {
        let gate = (mode == YamlViewMode::ApplyClean)
            .then(|| self.draft_gate.lock().unwrap().take())
            .flatten();
        if mode != YamlViewMode::ApplyClean {
            self.view_reads.fetch_add(1, Ordering::SeqCst);
        }
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            Ok(if resource.kind == "Secret" {
                super::adapters::SECRET_YAML
            } else {
                super::YAML
            }
            .into())
        })
    }

    pub fn prepare_yaml_apply(&self, request: YamlApplyRequest) -> Read<YamlApplyPreview> {
        let fail = self.dry_run_fail.load(Ordering::SeqCst);
        let gate = self.dry_run_gate.lock().unwrap().take();
        Box::pin(async move {
            if let Some(gate) = gate {
                let _ = gate.await;
            }
            if fail {
                return Err(AppError::new(
                    "spec.selector is immutable",
                    AppErrorKind::ImmutableField,
                ));
            }
            Ok(YamlApplyPreview {
                target: target(&request),
                current_yaml: "metadata:\n  name: payments-api\n".into(),
                dry_run_yaml: request.yaml,
                uid: "payments-api-uid".into(),
                resource_version: "7".into(),
            })
        })
    }

    pub fn apply_yaml(&self, request: YamlApplyRequest) -> Read<YamlApplyResult> {
        self.applied.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            Ok(YamlApplyResult {
                target: target(&request),
                applied_yaml: request.yaml,
            })
        })
    }
}

fn target(request: &YamlApplyRequest) -> kubecove_lib::models::YamlApplyTarget {
    kubecove_lib::models::YamlApplyTarget {
        cluster_context: request.cluster_context.clone(),
        kind: request.kind.clone(),
        api_version: request.api_version.clone(),
        name: request.name.clone(),
        namespace: request.namespace.clone(),
    }
}

/// Opens the Deployment's detail on `tab`.
fn open_tab(
    cx: &mut TestAppContext,
    backend: Arc<Controlled>,
    tab: &str,
) -> gpui_kit::AnyWindowHandle {
    let handle = open(cx, backend);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("resource:Deployment:payments:payments-api:mock-dev", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("show-{tab}"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    handle
}

fn step(
    cx: &mut TestAppContext,
    handle: gpui_kit::AnyWindowHandle,
    act: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App),
) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        act(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// Failure mode: a tab picked while the details read runs is dropped, so the
/// detail opens on Details.
#[gpui_kit::test]
fn a_tab_picked_while_the_detail_loads_is_kept(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (release, gate) = tokio::sync::oneshot::channel();
    *backend.detail.details_gate.lock().unwrap() = Some(gate);
    let handle = open(cx, backend);
    step(cx, handle, |window, cx| {
        window.click("resource:Deployment:payments:payments-api:mock-dev", cx);
    });
    step(cx, handle, |window, cx| {
        window.click("show-yaml", cx);
    });
    let _ = release.send(());
    cx.run_until_parked();
    step(cx, handle, |window, _| {
        assert_eq!(window.find("show-yaml").selected(), Some(true));
    });
}

#[gpui_kit::test]
fn a_refused_log_stream_is_visible(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.detail.log_fail.store(true, Ordering::SeqCst);
    let handle = open_tab(cx, backend, "logs");
    step(cx, handle, |window, _| {
        assert_eq!(
            window.find("log-error").label(),
            Some("Log stream failed: pods/log is forbidden")
        );
    });
}

/// Failure mode: the stream keeps running after its detail closes.
#[gpui_kit::test]
fn closing_the_detail_stops_its_log_stream(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open_tab(cx, backend.clone(), "logs");
    step(cx, handle, |window, cx| {
        assert_eq!(
            window.find(("log-row", 0usize)).label(),
            Some("boot payments-api")
        );
        assert!(!backend.detail.log_stopped.load(Ordering::SeqCst));
        window.click("close-detail", cx);
    });
    assert!(backend.detail.log_stopped.load(Ordering::SeqCst));
}

/// Failure mode: a refused dry run still offers to execute the action.
#[gpui_kit::test]
fn a_refused_dry_run_never_executes(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.detail.preview_fail.store(true, Ordering::SeqCst);
    let handle = open_tab(cx, backend.clone(), "actions");
    step(cx, handle, |window, cx| window.click("action-restart", cx));
    step(cx, handle, |window, _| {
        let error = window.find("review-error").label().unwrap().to_owned();
        assert!(
            error.ends_with("dry run denied by admission webhook"),
            "{error}"
        );
        assert!(!error.contains("Refresh this resource"));
        assert!(window.try_find("review-execute").is_none());
    });
    assert_eq!(backend.detail.executed.load(Ordering::SeqCst), 0);
}

/// Failure mode: a failed execution closes the review as if it had applied.
#[gpui_kit::test]
fn a_failed_execution_keeps_the_review_open(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.detail.execute_fail.store(true, Ordering::SeqCst);
    let handle = open_tab(cx, backend.clone(), "actions");
    step(cx, handle, |window, cx| window.click("action-restart", cx));
    step(cx, handle, |window, cx| window.click("review-execute", cx));
    step(cx, handle, |window, _| {
        assert!(window.find("review-title").visible());
        let error = window.find("review-error").label().unwrap().to_owned();
        assert!(
            error.contains("Refresh this resource before retrying. resourceVersion changed"),
            "{error}"
        );
        assert!(window.try_find("action-result").is_none());
    });
    assert_eq!(backend.detail.executed.load(Ordering::SeqCst), 1);
}

pub fn edit_draft(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, yaml: &str) {
    let editor = window
        .find_by_label("Resource YAML")
        .path()
        .last()
        .unwrap()
        .clone();
    window.within("detail").click(editor, cx);
    window.press(
        if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        },
        cx,
    );
    window.input(yaml, cx);
    window.render_frame(cx);
    assert_eq!(window.find_by_label("Resource YAML").value(), Some(yaml));
}

const DRAFT: &str = "kind: Deployment\nmetadata:\n  name: payments-api\n  labels:\n    tier: web\n";

#[gpui_kit::test]
fn a_refused_yaml_dry_run_shows_its_error_and_blocks_apply(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.detail.dry_run_fail.store(true, Ordering::SeqCst);
    let handle = open_tab(cx, backend.clone(), "yaml");
    step(cx, handle, |window, cx| window.click("edit-draft", cx));
    step(cx, handle, |window, cx| edit_draft(window, cx, DRAFT));
    step(cx, handle, |window, cx| window.click("yaml-dry-run", cx));
    step(cx, handle, |window, cx| {
        assert!(
            window
                .find_by_label("This field cannot be changed in place")
                .visible()
        );
        window.click("yaml-apply", cx);
    });
    assert_eq!(backend.detail.applied.load(Ordering::SeqCst), 0);
}

/// Failure mode: returning to the YAML tab while the edit draft loads reads the
/// view in the draft's place, so the draft never arrives and Edit stays disabled.
#[gpui_kit::test]
fn returning_to_the_yaml_tab_keeps_the_draft_load(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open_tab(cx, backend.clone(), "yaml");
    let (release, gate) = tokio::sync::oneshot::channel();
    *backend.detail.draft_gate.lock().unwrap() = Some(gate);
    step(cx, handle, |window, cx| window.click("edit-draft", cx));
    step(cx, handle, |window, cx| window.click("show-events", cx));
    step(cx, handle, |window, cx| window.click("show-yaml", cx));
    let _ = release.send(());
    cx.run_until_parked();
    step(cx, handle, |window, _| {
        assert!(window.find("discard-draft").visible());
    });
}

/// Failure mode: a dry run that finishes after the draft changed marks the
/// new, unreviewed text as reviewed.
#[gpui_kit::test]
fn a_dry_run_of_an_old_draft_is_dropped(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (release, gate) = tokio::sync::oneshot::channel();
    *backend.detail.dry_run_gate.lock().unwrap() = Some(gate);
    let handle = open_tab(cx, backend.clone(), "yaml");
    step(cx, handle, |window, cx| window.click("edit-draft", cx));
    step(cx, handle, |window, cx| edit_draft(window, cx, DRAFT));
    step(cx, handle, |window, cx| window.click("yaml-dry-run", cx));
    step(cx, handle, |window, cx| {
        edit_draft(
            window,
            cx,
            "kind: Deployment\nmetadata:\n  name: payments-api\n",
        )
    });
    assert!(
        release.send(()).is_err(),
        "the old dry run must release its gate when the draft changes"
    );
    step(cx, handle, |window, cx| {
        assert_eq!(window.find("yaml-apply").label(), Some("Apply"));
        assert_eq!(
            window.find("yaml-apply").description(),
            Some("Run a dry run before applying.")
        );
        window.click("yaml-apply", cx);
    });
    assert_eq!(backend.detail.applied.load(Ordering::SeqCst), 0);
    step(cx, handle, |window, cx| window.click("yaml-dry-run", cx));
    step(cx, handle, |window, cx| window.click("yaml-apply", cx));
    assert_eq!(backend.detail.applied.load(Ordering::SeqCst), 1);
    step(cx, handle, |window, _| {
        assert_eq!(
            window.find("yaml-applied").label(),
            Some("Deployment/payments-api applied.")
        );
    });
}

fn revision(name: &str, number: u64, image: &str) -> DeploymentRevision {
    DeploymentRevision {
        name: name.into(),
        revision: Some(number),
        change_cause: None,
        created_at: None,
        pod_template_yaml: format!("image: {image}\n"),
    }
}

/// Failure modes: a failed revisions read never retries, so the tab stays on its
/// error until the detail closes; and the comparison picker keyed by label
/// resolves two ReplicaSets with the same revision number to the first one.
#[gpui_kit::test]
fn revisions_retry_after_a_failure_and_compare_by_replica_set(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.detail.revisions.lock().unwrap().extend([
        Err(AppError::new(
            "replicasets is forbidden",
            AppErrorKind::Forbidden,
        )),
        Ok(vec![
            revision("payments-api-top", 4, "top"),
            revision("payments-api-a", 3, "first"),
            revision("payments-api-b", 3, "second"),
        ]),
    ]);
    let handle = open_tab(cx, backend.clone(), "revisions");
    step(cx, handle, |window, cx| {
        assert!(window.find("revisions-error").visible());
        window.click("show-details", cx);
    });
    step(cx, handle, |window, cx| window.click("show-revisions", cx));
    step(cx, handle, |window, cx| {
        let diff = window
            .find("revision-diff")
            .label()
            .unwrap_or_default()
            .to_owned();
        assert!(diff.contains("image: first"), "{diff}");
        window.within("revision-comparison").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    });
    step(cx, handle, |window, _| {
        let diff = window
            .find("revision-diff")
            .label()
            .unwrap_or_default()
            .to_owned();
        assert!(diff.contains("image: second"), "{diff}");
    });
}

/// Failure mode: a sidebar row refused by the draft guard still moves the
/// sidebar selection and RBAC view, so the tree shows a target the draft never left.
#[gpui_kit::test]
fn a_refused_sidebar_row_keeps_its_selection(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open_tab(cx, backend, "yaml");
    step(cx, handle, |window, cx| window.click("edit-draft", cx));
    step(cx, handle, |window, cx| edit_draft(window, cx, DRAFT));
    step(cx, handle, |window, cx| window.click("nav:RBAC", cx));
    step(cx, handle, |window, _| {
        assert_eq!(window.find("nav:RBAC").description(), Some("unselected"));
        assert!(window.find("discard-draft").visible());
    });
}

/// Failure mode: a successful apply shows its message beside the pre-edit
/// manifest, because nothing reads the applied object's YAML again.
#[gpui_kit::test]
fn a_successful_apply_reads_the_yaml_view_again(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open_tab(cx, backend.clone(), "yaml");
    step(cx, handle, |window, cx| window.click("edit-draft", cx));
    step(cx, handle, |window, cx| edit_draft(window, cx, DRAFT));
    step(cx, handle, |window, cx| window.click("yaml-dry-run", cx));
    let before = backend.detail.view_reads.load(Ordering::SeqCst);
    step(cx, handle, |window, cx| window.click("yaml-apply", cx));
    assert_eq!(backend.detail.applied.load(Ordering::SeqCst), 1);
    assert!(
        backend.detail.view_reads.load(Ordering::SeqCst) > before,
        "the applied object's YAML must be read again"
    );
    step(cx, handle, |window, _| {
        assert_eq!(
            window.find("yaml-applied").label(),
            Some("Deployment/payments-api applied.")
        );
    });
}
