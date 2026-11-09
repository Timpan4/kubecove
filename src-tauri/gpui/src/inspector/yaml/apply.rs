//! Guarded YAML apply flow of `ResourceYamlPane.svelte` (ADR 0004). The draft
//! is the server's apply-clean YAML, a server-side dry run reviews it, and
//! Apply sends only the reviewed request. A phase that waits on a read owns
//! it, so a new step, Cancel or closing the panel cancels the step it leaves.
//! `tab` renders this state.

use super::super::Inspector;
use super::super::detail::{Detail, ResourceDetail};
use super::super::request::InFlight;
use crate::unified_diff::{self, DiffLine};
use gpui_kit::{Context, Task, Window};
use kubecove_lib::models::{
    AppError, ResourceSummary, YamlApplyPreview, YamlApplyRequest, YamlViewMode,
};

pub(super) struct Review {
    pub request: YamlApplyRequest,
    pub lines: Vec<DiffLine>,
    pub full_lines: Vec<DiffLine>,
    /// The diff shows every line instead of its compact form.
    pub show_full: bool,
}

pub(super) enum Phase {
    Viewing,
    /// The apply-clean draft read runs.
    LoadingDraft {
        _read: InFlight,
    },
    Editing,
    /// The dry run of the draft runs.
    Preparing {
        _read: InFlight,
    },
    Reviewed(Review),
    Applying {
        review: Review,
        _read: InFlight,
    },
}

pub(super) enum Feedback {
    None,
    FormatFailed(String),
    PrepareFailed(AppError),
    ApplyFailed(String),
    Applied(String),
}

pub(in crate::inspector) struct YamlApply {
    phase: Phase,
    /// A view reload while `Viewing`, or a Format while `Editing` or `Reviewed`.
    /// Neither changes the phase, and entering a phase cancels it.
    beside: Option<InFlight>,
    pub(super) feedback: Feedback,
    /// The apply-clean draft the editor started from, which `dirty` compares.
    pub(super) base: String,
    pub(super) force_for_resource: bool,
}

impl Default for YamlApply {
    fn default() -> Self {
        Self {
            phase: Phase::Viewing,
            beside: None,
            feedback: Feedback::None,
            base: String::new(),
            force_for_resource: false,
        }
    }
}

impl YamlApply {
    pub(super) fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Moves to `phase`. The phase it leaves drops its read, and so does a
    /// reload or Format that ran beside it.
    fn enter(&mut self, phase: Phase) {
        self.beside = None;
        self.phase = phase;
    }

    /// Runs a view reload or a Format beside the current phase, in place of the last one.
    pub(super) fn run_beside(&mut self, read: Task<()>) {
        self.beside = Some(read.into());
    }

    pub(super) fn toggle_full_diff(&mut self) {
        if let Phase::Reviewed(review) | Phase::Applying { review, .. } = &mut self.phase {
            review.show_full = !review.show_full;
        }
    }

    pub(super) fn feedback_format(&self) -> Option<String> {
        if let Feedback::FormatFailed(message) = &self.feedback {
            Some(message.clone())
        } else {
            None
        }
    }

    /// `yamlEditing`: the draft editor replaces the read-only view.
    pub(super) fn editing(&self) -> bool {
        !matches!(self.phase, Phase::Viewing | Phase::LoadingDraft { .. })
    }

    /// The editor's `draft` differs from the YAML the edit started from.
    pub fn dirty(&self, draft: &str) -> bool {
        self.editing() && draft != self.base
    }

    pub(super) fn review(&self) -> Option<&Review> {
        match &self.phase {
            Phase::Reviewed(review) | Phase::Applying { review, .. } => Some(review),
            _ => None,
        }
    }

    /// `clearYamlDraftFeedback`: any draft change invalidates the dry run. A
    /// pending dry run reviewed the old text, so entering `Editing` cancels it.
    pub fn draft_changed(&mut self) {
        if !self.editing() || matches!(self.phase, Phase::Applying { .. }) {
            return;
        }
        self.enter(Phase::Editing);
        self.feedback = Feedback::None;
        self.force_for_resource = false;
    }
}

/// `isYamlApplyDisabled`.
pub(super) fn disabled_reason(resource: &ResourceSummary) -> Option<&'static str> {
    (resource.kind == "Secret" && resource.api_version.as_deref().unwrap_or("v1") == "v1")
        .then_some("Secret YAML apply is disabled to avoid exposing sensitive data.")
}

/// `buildYamlApplyRequest` for the exact selected resource.
pub(super) fn apply_request(
    resource: &ResourceSummary,
    yaml: String,
    force: bool,
) -> YamlApplyRequest {
    YamlApplyRequest {
        cluster_context: resource.cluster.clone(),
        kubeconfig_env_var: None,
        kind: resource.kind.clone(),
        api_version: resource.api_version.clone(),
        group: resource.group.clone(),
        version: resource.version.clone(),
        plural: resource.plural.clone(),
        namespaced: resource.namespaced,
        name: resource.name.clone(),
        namespace: resource.namespace.clone(),
        yaml,
        yaml_encoding: Default::default(),
        force_conflicts: force,
    }
}

fn review(
    request: YamlApplyRequest,
    preview: &YamlApplyPreview,
    style: crate::preferences::DiffStyle,
) -> Review {
    let (current, dry_run) = (&preview.current_yaml, &preview.dry_run_yaml);
    let force = request.force_conflicts;
    Review {
        show_full: false,
        lines: if style == crate::preferences::DiffStyle::Git {
            unified_diff::dry_run_git(current, dry_run, false)
        } else {
            unified_diff::dry_run(current, dry_run, false, force)
        },
        full_lines: if style == crate::preferences::DiffStyle::Git {
            unified_diff::dry_run_git(current, dry_run, true)
        } else {
            unified_diff::dry_run(current, dry_run, true, force)
        },
        request,
    }
}

impl Inspector {
    pub(super) fn yaml_apply(&mut self) -> Option<&mut ResourceDetail> {
        match &mut self.detail {
            Detail::Ready(detail) => Some(detail),
            _ => None,
        }
    }

    /// `resetYamlApply`: back to the read-only view of the loaded YAML.
    pub(super) fn reset_yaml_apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.yaml_apply() else {
            return;
        };
        detail.yaml = YamlApply::default();
        detail.lint.clear();
        let selected =
            gpui_kit::SharedString::from(if detail.yaml_view.mode == YamlViewMode::ApplyClean {
                "Apply clean"
            } else {
                "Kubectl view"
            });
        detail
            .yaml_view
            .shape
            .update(cx, |s, cx| s.set_selected_value(&selected, window, cx));
        let yaml = if detail.yaml_view.text.is_empty() {
            detail.data.yaml.clone()
        } else {
            detail.yaml_view.text.clone()
        };
        detail
            .editor
            .update(cx, |editor, cx| editor.set_value(yaml, window, cx));
        detail.marks.refresh(&detail.editor, cx);
        self.notice = None;
        self.refresh_yaml_lens(cx);
        cx.notify();
    }

    /// `startYamlApplyEdit`: loads the apply-clean draft from the server.
    pub(super) fn start_yaml_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        let Some(detail) = self.yaml_apply() else {
            return;
        };
        if disabled_reason(&detail.data.summary).is_some()
            || !matches!(detail.yaml.phase, Phase::Viewing)
        {
            return;
        }
        detail.yaml_view.shape.update(cx, |s, cx| {
            s.set_selected_value(&gpui_kit::SharedString::from("Apply clean"), window, cx)
        });
        let read = backend.resource_yaml_encoded(
            detail.data.summary.clone(),
            YamlViewMode::ApplyClean,
            detail.yaml_view.format,
        );
        let read = timings.measure("get_resource_yaml", read);
        let read = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(detail) = this.yaml_apply() else {
                    return;
                };
                match result {
                    Ok(draft) => {
                        detail.yaml.base = draft.clone();
                        detail
                            .editor
                            .update(cx, |editor, cx| editor.set_value(draft, window, cx));
                        detail.marks.refresh(&detail.editor, cx);
                        // Set after the value, so loading the draft is not a draft change.
                        detail.yaml.enter(Phase::Editing);
                    }
                    Err(error) => {
                        detail.yaml.enter(Phase::Viewing);
                        detail.yaml.feedback = Feedback::PrepareFailed(error);
                    }
                }
                this.schedule_yaml_lint(window, cx);
                cx.notify();
            });
        });
        detail.yaml = YamlApply {
            phase: Phase::LoadingDraft { _read: read.into() },
            ..Default::default()
        };
        cx.notify();
    }

    /// `previewYamlApply`: a server-side dry run of the current draft.
    pub(super) fn preview_yaml_apply(
        &mut self,
        force: Option<bool>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        let global_force = self.persisted.preferences.allow_yaml_force_conflicts;
        let Some(detail) = self.yaml_apply() else {
            return;
        };
        if !matches!(detail.yaml.phase, Phase::Editing | Phase::Reviewed(_)) {
            return;
        }
        let draft = detail.editor.read(cx).value().to_string();
        let force = force.unwrap_or(detail.yaml.force_for_resource || global_force);
        let mut request = apply_request(&detail.data.summary, draft, force);
        request.yaml_encoding = detail.yaml_view.format;
        let read = backend.prepare_yaml_apply(request.clone());
        let read = timings.measure("prepare_yaml_apply", read);
        let read = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let style = this.persisted.preferences.yaml_diff_style;
                let Some(detail) = this.yaml_apply() else {
                    return;
                };
                match result {
                    Ok(preview) => {
                        detail
                            .yaml
                            .enter(Phase::Reviewed(review(request, &preview, style)));
                    }
                    Err(error) => {
                        detail.yaml.enter(Phase::Editing);
                        detail.yaml.feedback = Feedback::PrepareFailed(error);
                    }
                }
                if this.persisted.preferences.skip_change_review {
                    this.apply_yaml_review(window, cx);
                }
                cx.notify();
            });
        });
        detail.yaml.enter(Phase::Preparing { _read: read.into() });
        detail.yaml.feedback = Feedback::None;
        cx.notify();
    }

    /// `applyYamlPreview`: applies only the reviewed request for an unchanged draft.
    pub(super) fn apply_yaml_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        let Some(detail) = self.yaml_apply() else {
            return;
        };
        let draft = detail.editor.read(cx).value().to_string();
        let phase = std::mem::replace(&mut detail.yaml.phase, Phase::Viewing);
        let Phase::Reviewed(review) = phase else {
            detail.yaml.phase = phase;
            return;
        };
        if review.request.yaml != draft {
            detail.yaml.phase = Phase::Reviewed(review);
            return;
        }
        let read = backend.apply_yaml(review.request.clone());
        let read = timings.measure("apply_yaml", read);
        let force = review.request.force_conflicts;
        let read = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(applied) => {
                        let target = applied.target;
                        let forced = if force { " with force-conflicts" } else { "" };
                        let message = format!("{}/{} applied{forced}.", target.kind, target.name);
                        this.reset_yaml_apply(window, cx);
                        if let Some(detail) = this.yaml_apply() {
                            detail.yaml.feedback = Feedback::Applied(message);
                        }
                    }
                    Err(error) => {
                        let Some(detail) = this.yaml_apply() else {
                            return;
                        };
                        let draft = detail.editor.read(cx).value().to_string();
                        let phase = std::mem::replace(&mut detail.yaml.phase, Phase::Editing);
                        if let Phase::Applying { review, .. } = phase
                            && review.request.yaml == draft
                        {
                            detail.yaml.phase = Phase::Reviewed(review);
                        }
                        detail.yaml.feedback = Feedback::ApplyFailed(error.message);
                    }
                }
                cx.notify();
            });
        });
        detail.yaml.enter(Phase::Applying {
            review,
            _read: read.into(),
        });
        detail.yaml.feedback = Feedback::None;
        cx.notify();
    }
}
