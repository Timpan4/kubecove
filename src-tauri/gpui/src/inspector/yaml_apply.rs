//! Guarded YAML apply flow of `ResourceYamlPane.svelte` (ADR 0004). The draft
//! is the server's apply-clean YAML, a server-side dry run reviews it, and
//! Apply sends only the reviewed request. Async steps run in
//! `Inspector::diff_task`, so a new step, Cancel or closing the panel drops the
//! previous step's result. `yaml_tab` renders this state.

use super::Inspector;
use super::detail::{Detail, ResourceDetail};
use crate::unified_diff::{self, DiffLine};
use gpui_kit::{Context, Window};
use kubecove_lib::models::{
    AppError, ResourceSummary, YamlApplyPreview, YamlApplyRequest, YamlViewMode,
};

pub(super) struct Review {
    pub request: YamlApplyRequest,
    pub lines: Vec<DiffLine>,
    pub full_lines: Vec<DiffLine>,
}

pub(super) enum Phase {
    Viewing,
    LoadingDraft,
    Editing,
    Preparing,
    Reviewed(Review),
    Applying(Review),
}

pub(super) enum Feedback {
    None,
    PrepareFailed(AppError),
    ApplyFailed(String),
    Applied(String),
}

pub(super) struct YamlApply {
    pub phase: Phase,
    pub feedback: Feedback,
    /// The apply-clean draft the editor started from, which `dirty` compares.
    pub base: String,
    pub force_for_resource: bool,
    pub show_full: bool,
}

impl Default for YamlApply {
    fn default() -> Self {
        Self {
            phase: Phase::Viewing,
            feedback: Feedback::None,
            base: String::new(),
            force_for_resource: false,
            show_full: false,
        }
    }
}

impl YamlApply {
    /// `yamlEditing`: the draft editor replaces the read-only view.
    pub fn editing(&self) -> bool {
        !matches!(self.phase, Phase::Viewing | Phase::LoadingDraft)
    }

    pub fn review(&self) -> Option<&Review> {
        match &self.phase {
            Phase::Reviewed(review) | Phase::Applying(review) => Some(review),
            _ => None,
        }
    }

    /// `clearYamlDraftFeedback`: any draft change invalidates the dry run.
    /// Returns whether a pending step must be dropped.
    pub fn draft_changed(&mut self) -> bool {
        if !self.editing() || matches!(self.phase, Phase::Applying(_)) {
            return false;
        }
        self.phase = Phase::Editing;
        self.feedback = Feedback::None;
        self.force_for_resource = false;
        self.show_full = false;
        true
    }
}

/// `isYamlApplyDisabled`.
pub(super) fn disabled_reason(resource: &ResourceSummary) -> Option<&'static str> {
    (resource.kind == "Secret" && resource.api_version.as_deref().unwrap_or("v1") == "v1")
        .then_some("Secret YAML apply is disabled to avoid exposing sensitive data.")
}

/// `buildYamlApplyRequest` for the exact selected resource.
fn apply_request(resource: &ResourceSummary, yaml: String, force: bool) -> YamlApplyRequest {
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

fn review(request: YamlApplyRequest, preview: &YamlApplyPreview) -> Review {
    let (current, dry_run) = (&preview.current_yaml, &preview.dry_run_yaml);
    let force = request.force_conflicts;
    Review {
        lines: unified_diff::dry_run(current, dry_run, false, force),
        full_lines: unified_diff::dry_run(current, dry_run, true, force),
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
        self.diff_task.take();
        let Some(detail) = self.yaml_apply() else {
            return;
        };
        detail.yaml = YamlApply::default();
        let yaml = detail.data.yaml.clone();
        detail
            .editor
            .update(cx, |editor, cx| editor.set_value(yaml, window, cx));
        detail.marks.refresh(&detail.editor, cx);
        self.notice = None;
        cx.notify();
    }

    /// `startYamlApplyEdit`: loads the apply-clean draft from the server.
    pub(super) fn start_yaml_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let Some(detail) = self.yaml_apply() else {
            return;
        };
        if disabled_reason(&detail.data.summary).is_some()
            || !matches!(detail.yaml.phase, Phase::Viewing)
        {
            return;
        }
        detail.yaml = YamlApply {
            phase: Phase::LoadingDraft,
            ..Default::default()
        };
        let read = backend.resource_yaml(detail.data.summary.clone(), YamlViewMode::ApplyClean);
        self.diff_task = Some(cx.spawn_in(window, async move |this, cx| {
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
                        detail.yaml.phase = Phase::Editing;
                    }
                    Err(error) => {
                        detail.yaml.phase = Phase::Viewing;
                        detail.yaml.feedback = Feedback::PrepareFailed(error);
                    }
                }
                cx.notify();
            });
        }));
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
        let Some(detail) = self.yaml_apply() else {
            return;
        };
        if !matches!(detail.yaml.phase, Phase::Editing | Phase::Reviewed(_)) {
            return;
        }
        let draft = detail.editor.read(cx).value().to_string();
        let state = &mut detail.yaml;
        state.phase = Phase::Preparing;
        state.feedback = Feedback::None;
        state.show_full = false;
        let force = force.unwrap_or(state.force_for_resource);
        let request = apply_request(&detail.data.summary, draft, force);
        let read = backend.prepare_yaml_apply(request.clone());
        self.diff_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let Some(detail) = this.yaml_apply() else {
                    return;
                };
                detail.yaml.phase = match result {
                    Ok(preview) => Phase::Reviewed(review(request, &preview)),
                    Err(error) => {
                        detail.yaml.feedback = Feedback::PrepareFailed(error);
                        Phase::Editing
                    }
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// `applyYamlPreview`: applies only the reviewed request for an unchanged draft.
    pub(super) fn apply_yaml_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
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
        let force = review.request.force_conflicts;
        detail.yaml.phase = Phase::Applying(review);
        detail.yaml.feedback = Feedback::None;
        self.diff_task = Some(cx.spawn_in(window, async move |this, cx| {
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
                        if let Phase::Applying(review) = phase
                            && review.request.yaml == draft
                        {
                            detail.yaml.phase = Phase::Reviewed(review);
                        }
                        detail.yaml.feedback = Feedback::ApplyFailed(error.message);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
