//! One GitOps selection as a card or a list row, from the `VirtualCollection`
//! snippets in `GitOpsView.svelte`.

use super::controls::Variant;
use super::delivery_ui::{self as dui, CARD_HEIGHT};
use super::detail::DetailTab;
use super::gitops_tooltip;
use super::{Inspector, surface};
use crate::gitops_surface::{Selection, SourceMode};
use crate::targets::TestSupportExt;
use crate::theme::{HEADING, Palette, Tokens};
use crate::ui::{self, Tone};
use gpui_kit::{ClipboardItem, Context, Div, FontWeight, Hsla, SharedString, div, prelude::*, px};
use kubecove_lib::models::HealthAssessmentState;

mod list;
pub(super) use list::{list_header, list_row};

/// `gitOpsCardTone`: the `w-1` bar's color.
fn tone(selection: Selection) -> Hsla {
    match selection.health().state {
        HealthAssessmentState::Degraded => Tokens::destructive(),
        HealthAssessmentState::NeedsAttention => Palette::amber400(),
        HealthAssessmentState::Healthy => Palette::emerald400(),
        _ => Tokens::muted(),
    }
}

/// `gitOpsSourceIcon` and `gitOpsSourceIconClass`: icon, border and color.
fn source_icon(mode: SourceMode) -> (&'static str, Hsla, Hsla) {
    match mode {
        SourceMode::Git => ("git-branch", Palette::sky500(0.35), Palette::sky300()),
        SourceMode::Helm => ("package", Palette::violet500(0.35), Palette::violet300()),
        SourceMode::Multi => ("layers", Palette::amber500(0.35), Palette::amber300()),
        SourceMode::Plugin => ("plug", Palette::emerald500(0.35), Palette::emerald300()),
        SourceMode::Unknown => ("circle-help", Tokens::border(), Tokens::muted_foreground()),
    }
}

/// `size-7 rounded-md border bg-background/50` around a `size-3.5` icon.
fn source_badge(
    selection: Selection,
    mode: SourceMode,
    cx: &mut Context<Inspector>,
) -> impl IntoElement + use<> {
    let (icon, border, color) = source_icon(mode);
    let badge = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded(px(8.))
        .border_1()
        .border_color(border)
        .bg(Tokens::background().opacity(0.5))
        .child(ui::icon(icon, 14., color));
    gitops_tooltip::reveal(
        badge,
        format!("gitops-source:{}", selection.key()),
        gitops_tooltip::source(selection),
        "source details".to_owned(),
        cx,
    )
}

/// `gitOpsStatusClass`.
fn status_tone(status: &str) -> Tone {
    match status {
        "Synced" | "Healthy" => Tone::Success,
        "Degraded" | "Missing" => Tone::Error,
        "OutOfSync" | "Progressing" | "Unknown" => Tone::Warning,
        _ => Tone::Neutral,
    }
}

/// `CopyableText`: the name, then a `size-5` copy button.
fn name(selection: Selection, size: f32, line: f32, cx: &mut Context<Inspector>) -> Div {
    let value = selection.name().to_owned();
    div()
        .flex()
        .min_w_0()
        .items_center()
        .gap(px(4.))
        .child(
            ui::text(value.clone(), size, line)
                .font_family(HEADING)
                .font_weight(FontWeight::MEDIUM)
                .min_w_0()
                .truncate(),
        )
        .child(
            div()
                .id(SharedString::from(format!(
                    "gitops-copy:{}",
                    selection.key()
                )))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(20.))
                .rounded(px(4.))
                .cursor_pointer()
                .hover(|button| button.bg(Tokens::muted()))
                .child(ui::icon("copy", 12., Tokens::muted_foreground()))
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.stop_propagation();
                    cx.write_to_clipboard(ClipboardItem::new_string(value.clone()));
                })),
        )
}

impl Inspector {
    /// `openGitOpsSelection`: an Application opens its resources, anything else its detail.
    fn open_gitops_selection(
        &mut self,
        key: &str,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        let Some(data) = self.gitops.inventory() else {
            return;
        };
        let found = self
            .gitops_page
            .rows()
            .iter()
            .filter_map(|row| row.resolve(data))
            .find(|selection| selection.key() == key)
            .map(|selection| {
                (
                    match selection {
                        Selection::App(app) => Some(app.clone()),
                        _ => None,
                    },
                    selection.resource(),
                )
            });
        match found {
            Some((Some(app), _)) => self.open_application_resources(app, window, cx),
            Some((None, resource)) => self.open_resource_on(resource, None, window, cx),
            None => {}
        }
    }
}

/// The Details and View resources buttons.
fn actions(selection: Selection, resources_label: &str, cx: &mut Context<Inspector>) -> Div {
    let resource = selection.resource();
    let details_id = format!(
        "gitops-details:{}:{}:{}",
        selection.kind(),
        selection.namespace().unwrap_or_default(),
        selection.name()
    );
    let argo = selection.opens_resources();
    let details = dui::sm_button(Variant::Ghost, "Details")
        .id(SharedString::from(details_id))
        .test_support()
        .aria_label(selection.details_label())
        .on_click(cx.listener(move |this, _, window, cx| {
            cx.stop_propagation();
            let tab = argo.then_some(DetailTab::Argo);
            this.open_resource_on(resource.clone(), tab, window, cx);
        }));
    let application = match selection {
        Selection::App(app) => Some(app.clone()),
        _ => None,
    };
    div().flex().items_center().gap(px(8.)).child(details).when(
        selection.opens_resources(),
        |row| {
            row.child(
                dui::sm_button(Variant::Outline, resources_label)
                    .id(SharedString::from(format!(
                        "gitops-resources:{}",
                        selection.key()
                    )))
                    .test_support()
                    .ml_auto()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        if let Some(app) = &application {
                            this.open_application_resources(app.clone(), window, cx);
                        }
                    })),
            )
        },
    )
}

/// A `h-[18.5rem]` card: tone bar, name and source icon, health badge, source
/// line, facts and actions.
pub(super) fn card(
    selection: Selection,
    width: f32,
    selected: bool,
    cx: &mut Context<Inspector>,
) -> impl IntoElement + use<> {
    let source = selection.source_mode();
    let key = selection.key();
    let facts = selection.facts();
    let fact_rows: Vec<_> = facts
        .chunks(2)
        .map(|pair| {
            div()
                .flex()
                .gap(px(6.))
                .children(pair.iter().map(|(label, value)| {
                    let field = div()
                        .flex_1()
                        .flex_basis(px(0.))
                        .min_w_0()
                        .relative()
                        .px(px(10.))
                        .pt(px(12.))
                        .pb(px(8.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Tokens::border())
                        .bg(Tokens::background().opacity(0.3))
                        .child(
                            ui::medium(ui::muted(ui::text(*label, 9.92, 9.92)))
                                .absolute()
                                .top(px(-7.))
                                .left(px(8.))
                                .px(px(4.))
                                .bg(Tokens::card()),
                        )
                        .child(ui::medium(ui::text(value.clone(), 12., 19.5)).truncate());
                    let text = if *label == "Revision" {
                        gitops_tooltip::revision(selection)
                    } else {
                        value.clone()
                    };
                    gitops_tooltip::reveal(
                        field,
                        format!("gitops-fact:{}:{label}", selection.key()),
                        text,
                        label.to_lowercase(),
                        cx,
                    )
                }))
                .when(pair.len() == 1, |row| {
                    row.child(div().flex_1().flex_basis(px(0.)))
                })
        })
        .collect();
    let health = selection.health();
    div()
        .id(SharedString::from(format!("gitops-card:{key}")))
        .test_support()
        .aria_label(selection.action_label())
        .relative()
        .flex()
        .flex_none()
        .w(px(width))
        .h(px(CARD_HEIGHT))
        .overflow_hidden()
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .cursor_pointer()
        // The focused row, as `border-primary/60` marks it in the web.
        .when(selected, |card| {
            card.border_color(Tokens::primary().opacity(0.6)).child(
                div()
                    .id("gitops-selected")
                    .test_support()
                    .aria_label(selection.name().to_string())
                    .absolute()
                    .w(px(0.))
                    .h(px(0.)),
            )
        })
        .hover(|card| {
            card.border_color(Tokens::primary().opacity(0.5))
                .shadow_md()
        })
        .on_click(
            cx.listener(move |this, _, window, cx| this.open_gitops_selection(&key, window, cx)),
        )
        .child(
            div()
                .w(px(4.))
                .flex_none()
                .rounded_l(px(10.))
                .bg(tone(selection)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .p(px(12.))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.))
                                .child(name(selection, 14., 20., cx))
                                .child(
                                    ui::muted(ui::text(selection.subtitle().to_owned(), 12., 19.5))
                                        .truncate(),
                                ),
                        )
                        .when_some(source, |row, mode| {
                            row.child(source_badge(selection, mode, cx))
                        }),
                )
                .child(
                    div()
                        .mt(px(12.))
                        .flex()
                        .gap(px(6.))
                        .child(surface::health_state_badge(health.state))
                        .when(
                            health.completeness
                                == kubecove_lib::models::HealthAssessmentCompleteness::Partial,
                            |row| row.child(ui::outline_badge("Partial").border_dashed()),
                        ),
                )
                .when_some(selection.source_line(), |column, line| {
                    column.child(gitops_tooltip::reveal(
                        ui::muted(ui::xs(line))
                            .mt(px(12.))
                            .w_full()
                            .truncate()
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::background().opacity(0.4)),
                        format!("gitops-source-line:{}", selection.key()),
                        gitops_tooltip::source(selection),
                        "source details".to_owned(),
                        cx,
                    ))
                })
                .child(
                    div()
                        .mt(px(12.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .children(fact_rows),
                )
                .child(
                    div()
                        .mt_auto()
                        .pt(px(12.))
                        .child(actions(selection, "View resources", cx)),
                ),
        )
}
