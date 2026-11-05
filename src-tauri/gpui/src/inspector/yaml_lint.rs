//! Kubernetes YAML lint of the editor draft, from `kubernetesYamlDiagnostics`
//! in `ResourceYamlPane.svelte`. A draft change clears the previous result,
//! and one lint runs after the 450 ms linter delay of `YamlCodeEditor.svelte`.
//! A newer change drops the pending lint, as the web linter restarts.
//! `yaml_tab` renders the result and `yaml_lens` marks it in the editor.
use super::Inspector;
use super::yaml_apply::apply_request;
use gpui_kit::{Context, Task, Window};
use kubecove_lib::models::{KubernetesYamlLintDiagnostic, KubernetesYamlLintStatusNote};
use std::time::Duration;

/// The `linter` delay in `YamlCodeEditor.svelte`.
const LINT_DELAY: Duration = Duration::from_millis(450);

/// The lint of the current draft. `task` holds the pending delay and request.
#[derive(Default)]
pub(super) struct Lint {
    pub diagnostics: Vec<KubernetesYamlLintDiagnostic>,
    pub notes: Vec<KubernetesYamlLintStatusNote>,
    pub error: Option<String>,
    task: Option<Task<()>>,
}

impl Lint {
    /// `clearYamlLint`: drops the result and any pending lint.
    pub fn clear(&mut self) {
        self.diagnostics.clear();
        self.notes.clear();
        self.error = None;
        self.task = None;
    }
}

impl Inspector {
    /// Clears the lint, then lints the draft after the delay while editing.
    pub(super) fn schedule_yaml_lint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        let Some(detail) = self.yaml_apply() else {
            return;
        };
        detail.lint.clear();
        let draft = detail.editor.read(cx).value().to_string();
        if detail.yaml.editing() && !draft.trim().is_empty() {
            let mut request = apply_request(&detail.data.summary, draft, false);
            request.yaml_encoding = detail.yaml_view.format;
            let timer = cx.background_executor().timer(LINT_DELAY);
            detail.lint.task = Some(cx.spawn_in(window, async move |this, cx| {
                timer.await;
                let read = timings.measure("lint_kubernetes_yaml", backend.lint_yaml(request));
                let result = read.await;
                let _ = this.update_in(cx, |this, _window, cx| {
                    if let Some(detail) = this.yaml_apply() {
                        match result {
                            Ok(lint) => {
                                detail.lint.diagnostics = lint.diagnostics;
                                detail.lint.notes = lint.notes;
                                detail.lint.error = None;
                            }
                            Err(error) => {
                                detail.lint.diagnostics.clear();
                                detail.lint.notes.clear();
                                detail.lint.error = Some(error.message);
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
