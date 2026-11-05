//! YAML tab of the resource detail panel, mirroring `YamlTab.svelte` and the
//! draft editor of `ResourceYamlPane.svelte`. `yaml_apply` owns the guarded
//! apply flow this view drives.

use super::Inspector;
use super::controls::{Variant, button, diff_lines};
use super::detail::ResourceDetail;
use super::yaml_apply::{Feedback, Phase, Review, YamlApply, disabled_reason};
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::base::ObservedElement;
use gpui_kit::component::input::Editor;
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, SharedString, Stateful, TestSupportExt, div, prelude::*,
    px,
};
use kubecove_lib::models::{
    AppErrorKind, KubernetesYamlLintDiagnostic, KubernetesYamlLintSeverity,
    KubernetesYamlLintStatusNote, ResourceSummary,
};

/// Lines the compact dry-run diff shows (`yamlDiffLines.slice(0, 24)`).
const COMPACT_DIFF_LINES: usize = 24;

/// `FriendlyError` in its unclassified form: the title, the generic summary
/// and the technical detail, which the native client shows expanded.
fn failure(id: &'static str, title: &str, detail: String) -> ObservedElement<Stateful<Div>> {
    div()
        .id(id)
        .test_support()
        .role(gpui_kit::Role::Alert)
        .aria_label(title.to_owned())
        .aria_description(detail.clone())
        .flex()
        .flex_col()
        .gap(px(6.))
        .p(px(12.))
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::destructive().opacity(0.4))
        .bg(Tokens::card())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::icon("circle-alert", 16., Tokens::destructive()))
                .child(ui::medium(ui::sm(title.to_owned())).text_color(Tokens::destructive())),
        )
        .child(
            ui::xs("The original technical error is still available below.")
                .text_color(Tokens::foreground().opacity(0.8)),
        )
        .child(
            ui::text(detail, 11., 17.875)
                .p(px(8.))
                .rounded(px(4.))
                .bg(Tokens::muted().opacity(0.4))
                .text_color(Tokens::muted_foreground()),
        )
}

fn operation_scope(resource: &ResourceSummary) -> ObservedElement<Stateful<Div>> {
    let line = |text: String| ui::xs(text).font_family(crate::theme::MONO);
    div()
        .id("yaml-apply-scope")
        .test_support()
        .aria_label(format!(
            "{} {} {} {}",
            resource.cluster,
            resource.namespace.as_deref().unwrap_or("cluster-scoped"),
            resource.kind,
            resource.name
        ))
        .grid()
        .grid_cols(2)
        .gap(px(4.))
        .p(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.5))
        .child(line(
            "Operation scope: Selected Kubernetes resource YAML".into(),
        ))
        .child(line(format!("Context: {}", resource.cluster)))
        .child(line(format!(
            "Namespace: {}",
            resource.namespace.as_deref().unwrap_or("cluster-scoped")
        )))
        .child(line(format!("Kind: {}", resource.kind)))
        .child(line(format!("Resource: {}", resource.name)))
}

fn force_conflicts(state: &YamlApply, global: bool) -> Div {
    let reviewed = state.review().map(|review| review.request.force_conflicts);
    let enabled = reviewed.unwrap_or(state.force_for_resource || global);
    let status = match reviewed {
        Some(true) => "enabled",
        Some(false) => "disabled",
        None if enabled => "enabled for next dry run",
        None => "disabled",
    };
    let source = if state.force_for_resource {
        "Current-resource override"
    } else if global {
        "Global preference"
    } else {
        "Not enabled"
    };
    let review = if enabled {
        "Enabled. If fields conflict, applying this reviewed result takes ownership of those fields from other field managers."
    } else {
        "Disabled. Existing field ownership remains enforced."
    };
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .p(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.5))
        .child(
            div()
                .flex()
                .gap(px(8.))
                .child(
                    ui::xs(format!("Force-conflicts: {status}"))
                        .flex_1()
                        .min_w_0(),
                )
                .child(
                    ui::xs(format!("Preference source: {source}"))
                        .flex_1()
                        .min_w_0(),
                ),
        )
        .child(ui::muted(ui::xs(review)))
}

fn dry_run_diff(
    review: &Review,
    full: bool,
    style: crate::preferences::DiffStyle,
    cx: &mut Context<Inspector>,
) -> Div {
    let lines = if full {
        &review.full_lines
    } else {
        &review.lines
    };
    let shown = if full {
        &lines[..]
    } else {
        &lines[..lines.len().min(COMPACT_DIFF_LINES)]
    };
    let hidden = lines.len() - shown.len();
    let text: SharedString = shown
        .iter()
        .map(|line| line.text.as_ref())
        .collect::<Vec<_>>()
        .join("\n")
        .into();
    div()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.5))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .px(px(12.))
                .py(px(8.))
                .border_b_1()
                .border_color(Tokens::border())
                .child(ui::medium(ui::xs(format!(
                    "Dry-run diff ({})",
                    if full { "full" } else { "compact" }
                ))))
                .child(
                    button(Variant::Ghost, 24., false)
                        .id("yaml-diff-toggle")
                        .test_support()
                        .child(if full {
                            "Collapse diff"
                        } else {
                            "Show full diff"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(detail) = this.yaml_apply() {
                                detail.yaml.show_full = !detail.yaml.show_full;
                            }
                            cx.notify();
                        })),
                ),
        )
        .child(
            div()
                .id("yaml-dry-run-diff")
                .test_support()
                .aria_label(text)
                .max_h(px(288.))
                .overflow_y_scroll()
                .py(px(4.))
                .child({
                    let clean: Vec<_> = shown
                        .iter()
                        .cloned()
                        .map(|mut line| {
                            line.text = line
                                .text
                                .strip_prefix(['+', '-', ' '])
                                .unwrap_or(&line.text)
                                .to_owned()
                                .into();
                            line
                        })
                        .collect();
                    diff_lines(
                        if style == crate::preferences::DiffStyle::Git {
                            shown
                        } else {
                            &clean
                        },
                        19.5,
                    )
                    .into_any_element()
                }),
        )
        .when(hidden > 0, |pane| {
            pane.child(
                ui::muted(ui::xs(format!("{hidden} hidden diff lines")))
                    .px(px(12.))
                    .py(px(8.))
                    .border_t_1()
                    .border_color(Tokens::border()),
            )
        })
}

fn edit_controls(
    state: &YamlApply,
    draft_empty: bool,
    kyaml: bool,
    cx: &mut Context<Inspector>,
) -> Div {
    let preparing = matches!(state.phase, Phase::Preparing);
    let applying = matches!(state.phase, Phase::Applying(_));
    let reviewed = state.review().is_some();
    let dry_run_disabled = preparing || applying || draft_empty;
    let apply_disabled = !reviewed || applying;
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_end()
        .gap(px(8.))
        .child(
            button(Variant::Outline, 28., preparing || applying)
                .id("yaml-format")
                .test_support()
                .child(ui::icon("wand-sparkles", 14., Tokens::foreground()))
                .aria_label(if kyaml {
                    "Format (KYAML)"
                } else {
                    "Format (YAML)"
                })
                .child(if kyaml {
                    "Format (KYAML)"
                } else {
                    "Format (YAML)"
                })
                .on_click(cx.listener(|this, _, window, cx| this.format_yaml(window, cx))),
        )
        .child({
            let dry_run = button(Variant::Secondary, 28., dry_run_disabled)
                .id("yaml-dry-run")
                .test_support()
                .aria_label("Dry run")
                .when(preparing, |button| {
                    button.child(ui::icon("loader-circle", 14., Tokens::foreground()))
                })
                .child("Dry run");
            if dry_run_disabled {
                dry_run
            } else {
                dry_run.on_click(
                    cx.listener(|this, _, window, cx| this.preview_yaml_apply(None, window, cx)),
                )
            }
        })
        .child({
            let apply = button(Variant::Default, 28., apply_disabled)
                .id("yaml-apply")
                .test_support()
                .aria_label(if reviewed {
                    "Apply"
                } else {
                    "Apply (run a dry run before applying)"
                })
                .when(applying, |button| {
                    button.child(ui::icon("loader-circle", 14., Tokens::primary_foreground()))
                })
                .child("Apply");
            if apply_disabled {
                apply
            } else {
                apply
                    .on_click(cx.listener(|this, _, window, cx| this.apply_yaml_review(window, cx)))
            }
        })
}

/// `YAML lint status`: the lint's notes, as the web alert lists them.
fn lint_status(notes: &[KubernetesYamlLintStatusNote]) -> Option<AnyElement> {
    (!notes.is_empty()).then(|| {
        div()
            .id("yaml-lint-status")
            .test_support()
            .aria_label("YAML lint status")
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(12.))
            .rounded(px(10.))
            .border_1()
            .border_color(Tokens::border())
            .child(ui::medium(ui::sm("YAML lint status")))
            .children(
                notes
                    .iter()
                    .map(|note| ui::xs(format!("{}: {}", note.source, note.message))),
            )
            .into_any_element()
    })
}

/// `YAML diagnostics`: each Kubernetes lint finding with its severity and field.
fn lint_diagnostics(diagnostics: &[KubernetesYamlLintDiagnostic]) -> Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .child(
            div()
                .px(px(12.))
                .py(px(8.))
                .border_b_1()
                .border_color(Tokens::border())
                .child(ui::medium(ui::xs("YAML diagnostics"))),
        )
        .children(diagnostics.iter().enumerate().map(|(index, diagnostic)| {
            let detail = match &diagnostic.field_path {
                Some(path) => format!("{} - {path}", diagnostic.source),
                None => diagnostic.source.clone(),
            };
            let severity = match diagnostic.severity {
                KubernetesYamlLintSeverity::Error => "error",
                KubernetesYamlLintSeverity::Warning => "warning",
                KubernetesYamlLintSeverity::Info => "info",
            };
            div()
                .id(SharedString::from(format!("yaml-diagnostic:{index}")))
                .test_support()
                .aria_label(diagnostic.message.clone())
                .aria_description(detail.clone())
                .flex()
                .gap(px(12.))
                .px(px(12.))
                .py(px(8.))
                .border_b_1()
                .border_color(Tokens::border())
                .child(
                    div()
                        .id(SharedString::from(format!(
                            "yaml-diagnostic-severity:{index}"
                        )))
                        .test_support()
                        .aria_label(severity)
                        .aria_description(detail.clone())
                        .flex_none()
                        .w(px(72.))
                        .child(ui::xs(severity)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(ui::medium(ui::xs(diagnostic.message.clone())))
                        .child(ui::muted(ui::xs(detail)).mt(px(4.))),
                )
        }))
}

pub(super) fn yaml_tab(
    detail: &ResourceDetail,
    preferences: &crate::preferences::Preferences,
    cx: &mut Context<Inspector>,
) -> Div {
    let style = preferences.yaml_diff_style;
    let summary = &detail.data.summary;
    let state = &detail.yaml;
    let editing = state.editing();
    let loading = matches!(state.phase, Phase::LoadingDraft);
    let applying = matches!(state.phase, Phase::Applying(_));
    let disabled = disabled_reason(summary);
    let path = [
        summary.cluster.as_str(),
        summary.namespace.as_deref().unwrap_or("cluster"),
        summary.kind.as_str(),
        summary.name.as_str(),
    ]
    .join(" / ");
    let applied = match &state.feedback {
        Feedback::Applied(message) => Some(message.clone()),
        _ => None,
    };
    let edit_button = if editing {
        button(Variant::Outline, 28., applying)
            .id("discard-draft")
            .test_support()
            .aria_label("Cancel")
            .child(ui::icon("x", 14., Tokens::foreground()))
            .child("Cancel")
            .when(!applying, |button| {
                button
                    .on_click(cx.listener(|this, _, window, cx| this.reset_yaml_apply(window, cx)))
            })
    } else {
        let blocked = disabled.is_some() || loading;
        button(Variant::Secondary, 28., blocked)
            .id("edit-draft")
            .test_support()
            .aria_label("Edit YAML")
            .when(loading, |button| {
                button.child(ui::icon("loader-circle", 14., Tokens::foreground()))
            })
            .child(ui::icon("pencil", 14., Tokens::foreground()))
            .child("Edit YAML")
            .when(!blocked, |button| {
                button.on_click(cx.listener(|this, _, window, cx| this.start_yaml_edit(window, cx)))
            })
    };
    let draft_empty = detail.editor.read(cx).value().trim().is_empty();
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap(px(12.))
        .when(
            summary.kind == "Secret" && !preferences.redact_secrets,
            |pane| pane.child(super::secret_data::view(detail, cx)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::outline_badge(path))
                        .when_some(applied, |row, message| {
                            row.child(
                                ui::xs(message.clone())
                                    .id("yaml-applied")
                                    .test_support()
                                    .aria_label(message)
                                    .px(px(8.))
                                    .rounded_full()
                                    .bg(Tokens::secondary())
                                    .font_weight(FontWeight::MEDIUM),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .flex_wrap()
                        .child(super::yaml_view::picker(
                            &detail.yaml_view,
                            true,
                            editing || loading,
                        ))
                        .child(super::yaml_view::picker(
                            &detail.yaml_view,
                            false,
                            editing || loading,
                        ))
                        .child(edit_button),
                )
                .when(editing, |column| {
                    column
                        .child(operation_scope(summary))
                        .child(force_conflicts(
                            state,
                            preferences.allow_yaml_force_conflicts,
                        ))
                        .child(edit_controls(
                            state,
                            draft_empty,
                            detail.yaml_view.format == kubecove_lib::models::YamlEncoding::Kyaml,
                            cx,
                        ))
                }),
        )
        .when_some(state.feedback_format(), |pane, message| {
            pane.child(failure("yaml-format-error", "Format failed", message))
        })
        .when_some(detail.yaml_view.error.clone(), |pane, message| {
            pane.child(failure("yaml-view-error", "Failed to load YAML", message))
        })
        .when_some(disabled, |pane, reason| {
            pane.child(
                div()
                    .id("yaml-apply-unavailable")
                    .test_support()
                    .aria_label("YAML apply unavailable")
                    .p(px(12.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(Tokens::border())
                    .child(ui::medium(ui::sm("YAML apply unavailable")))
                    .child(ui::muted(ui::xs(reason))),
            )
        })
        .when_some(detail.lint.error.clone(), |pane, message| {
            pane.child(failure("yaml-lint-error", "YAML lint failed", message))
        })
        .when_some(lint_status(&detail.lint.notes), |pane, status| {
            pane.child(status)
        })
        .when_some(
            match &state.feedback {
                Feedback::PrepareFailed(error) => Some(error),
                _ => None,
            },
            |pane, error| {
                let conflict =
                    error.kind == AppErrorKind::FieldManagerConflict && !state.force_for_resource;
                pane.child(failure(
                    "yaml-dry-run-error",
                    "Dry run failed",
                    error.message.clone(),
                ))
                .when(conflict, |pane| {
                    pane.child(
                        div().flex().child(
                            button(Variant::Destructive, 24., false)
                                .id("yaml-allow-force")
                                .test_support()
                                .child("Allow force-conflicts for this resource")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(detail) = this.yaml_apply() {
                                        detail.yaml.force_for_resource = true;
                                    }
                                    this.preview_yaml_apply(Some(true), window, cx)
                                })),
                        ),
                    )
                })
            },
        )
        .when_some(
            match &state.feedback {
                Feedback::ApplyFailed(message) => Some(message.clone()),
                _ => None,
            },
            |pane, message| pane.child(failure("yaml-apply-error", "YAML apply failed", message)),
        )
        .when_some(state.review().filter(|_| editing), |pane, review| {
            pane.child(dry_run_diff(review, state.show_full, style, cx))
        })
        // `YamlCodeEditor minHeight`: 520px read-only, 420px while editing. GPUI's
        // editor needs a definite height, so longer YAML scrolls inside it.
        .child(
            div()
                .relative()
                .h(px(if editing { 420. } else { 520. }))
                .flex_none()
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .overflow_hidden()
                .child(
                    Editor::new(&detail.editor)
                        .aria_label("Resource YAML")
                        .readonly(!editing || applying)
                        .size_full(),
                )
                .children(
                    detail
                        .lenses
                        .iter()
                        .map(|lens| super::yaml_lens::overlay(detail.editor.read(cx), lens)),
                ),
        )
        .when(editing && !detail.lint.diagnostics.is_empty(), |pane| {
            pane.child(lint_diagnostics(&detail.lint.diagnostics))
        })
}
