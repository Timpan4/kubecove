//! Kubernetes YAML lint of the editor draft, from `kubernetesYamlDiagnostics`
//! in `ResourceYamlPane.svelte`. A draft change clears the previous result,
//! and one lint runs after the 450 ms linter delay of `YamlCodeEditor.svelte`.
//! A newer change drops the pending lint, as the web linter restarts.
//! `tab` renders the result and `lens` marks it in the editor.
use super::super::Inspector;
use super::apply::apply_request;
use super::folds::YamlSyntax;
use super::lens::{Diagnostics, Lens};
use gpui_kit::component::input::EditorState;
use gpui_kit::{Context, Task, Window};
use kubecove_lib::models::{AppError, KubernetesYamlLintDiagnostic, KubernetesYamlLintStatusNote};
use std::time::Duration;

/// The `linter` delay in `YamlCodeEditor.svelte`.
const LINT_DELAY: Duration = Duration::from_millis(450);

/// The lint of the current draft. `task` holds the pending delay and request.
#[derive(Default)]
pub(in crate::inspector) struct Lint {
    pub(super) diagnostics: Vec<KubernetesYamlLintDiagnostic>,
    pub(super) notes: Vec<KubernetesYamlLintStatusNote>,
    /// The failure as the command returned it, kept whole so the view can
    /// classify it the way `FriendlyError` does.
    pub(super) error: Option<AppError>,
    task: Option<Task<()>>,
    syntax: YamlSyntax,
    marks: Diagnostics,
}

impl Lint {
    pub(in crate::inspector) fn new(syntax: YamlSyntax) -> Self {
        Self {
            syntax,
            ..Self::default()
        }
    }

    pub(super) fn update_diagnostics(
        &mut self,
        editor: &mut EditorState,
        enabled: bool,
        cx: &mut Context<EditorState>,
    ) -> Vec<Lens> {
        self.marks
            .update(&self.syntax, editor, enabled, &self.diagnostics, cx)
    }

    /// `clearYamlLint`: drops the result and any pending lint.
    pub(super) fn clear(&mut self) {
        self.diagnostics.clear();
        self.marks.clear_lint();
        self.notes.clear();
        self.error = None;
        self.task = None;
    }
}

impl Inspector {
    /// Clears the lint, then lints the draft after the delay while editing.
    pub(in crate::inspector) fn schedule_yaml_lint(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        let Some(detail) = self.yaml_apply() else {
            return;
        };
        detail.lint.clear();
        if detail.yaml.editing() {
            let draft = detail.editor.read(cx).text().to_string();
            if draft.trim().is_empty() {
                self.refresh_yaml_lens(cx);
                return;
            }
            let mut request = apply_request(&detail.data.summary, draft, false);
            request.yaml_encoding = detail.yaml_view.format;
            let timer = cx.background_executor().timer(LINT_DELAY);
            detail.lint.task = Some(cx.spawn_in(window, async move |this, cx| {
                timer.await;
                let read = timings.measure("lint_kubernetes_yaml", backend.lint_yaml(request));
                let result = read.await;
                let _ = this.update_in(cx, |this, _window, cx| {
                    if let Some(detail) = this.yaml_apply() {
                        detail.lint.marks.clear_lint();
                        match result {
                            Ok(lint) => {
                                detail.lint.diagnostics = lint.diagnostics;
                                detail.lint.notes = lint.notes;
                                detail.lint.error = None;
                            }
                            Err(error) => {
                                detail.lint.diagnostics.clear();
                                detail.lint.notes.clear();
                                detail.lint.error = Some(error);
                            }
                        }
                    }
                    this.refresh_yaml_lens(cx);
                    cx.notify();
                });
            }));
        }
        self.refresh_yaml_lens(cx);
    }
}
