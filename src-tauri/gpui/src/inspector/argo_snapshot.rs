//! The "Application snapshot" section of `ArgoApplicationDetails.svelte`: one
//! `<article>` per reconciliation resource, with `ArgoResourceDiff.svelte`
//! for the Changes view. Read-only YAML editors and diff lines share the
//! native YAML syntax highlighter.

use super::Inspector;
use super::argo_panels::{caps, panel};
use super::argo_tab::ArgoTab;
use super::controls::{self, Variant};
use super::delivery_ui as dui;
use super::loading::Phase;
use crate::argo_app::{ArgoView, Change, ChangeBadge};
use crate::theme::{MONO, Palette, Tokens};
use crate::ui::{self, Tone};
use crate::unified_diff::{DiffLine, DiffLineKind};
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, Div, FontWeight, SharedString, div, prelude::*, px};

/// `ArgoResourceDiff` shows this many lines until expanded.
const DIFF_PREVIEW: usize = 24;

/// `diffViewOptions`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DiffView {
    Changes,
    Target,
    Live,
}

const VIEWS: [(DiffView, &str); 3] = [
    (DiffView::Changes, "Changes"),
    (DiffView::Target, "Desired"),
    (DiffView::Live, "Live"),
];

fn toggle(list: &mut Vec<usize>, index: usize) {
    match list.iter().position(|item| *item == index) {
        Some(position) => {
            list.remove(position);
        }
        None => list.push(index),
    }
}

/// Applies `update` to the open tab's state and repaints.
fn update(this: &mut Inspector, cx: &mut Context<Inspector>, update: impl FnOnce(&mut ArgoTab)) {
    if let Some(state) = this.argo_tab_state() {
        update(state);
        cx.notify();
    }
}

/// `Button size="xs"`: `h-5 px-2 text-[0.625rem]`.
fn xs_button(variant: Variant, label: &str) -> Div {
    controls::button(variant, 20., false)
        .rounded(px(4.))
        .text_size(px(10.))
        .line_height(px(15.))
        .child(label.to_owned())
}

/// `SegmentedControl size="sm"` for the reconciliation document view.
fn segmented(active: DiffView, cx: &mut Context<Inspector>) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .p(px(2.))
        .rounded(px(10.))
        .bg(Tokens::background())
        .border_1()
        .border_color(Tokens::border().opacity(0.6))
        .children(VIEWS.map(|(option, label)| {
            let selected = option == active;
            div()
                .id(SharedString::from(format!("argo-view:{label}")))
                .test_support()
                .aria_label(label)
                .cursor_pointer()
                .flex()
                .items_center()
                .h(px(24.))
                .px(px(8.))
                .rounded(px(8.))
                .when(selected, |button| button.bg(Tokens::card()).shadow_sm())
                .child(ui::medium(ui::xs(label)).text_color(if selected {
                    Tokens::foreground()
                } else {
                    Tokens::muted_foreground()
                }))
                .on_click(
                    cx.listener(move |this, _, _, cx| {
                        update(this, cx, |state| state.view = option)
                    }),
                )
        }))
}

/// `border-t p-3 text-xs leading-relaxed`, with an optional medium title.
fn note(title: Option<&str>, text: &str) -> impl IntoElement {
    div()
        .id("argo-note")
        .test_support()
        .aria_label(title.unwrap_or(text).to_owned())
        .aria_description(text.to_owned())
        .p(px(12.))
        .border_t_1()
        .border_color(Tokens::border())
        .when_some(title, |note, title| {
            note.child(ui::medium(ui::text(title.to_owned(), 12., 19.5)))
        })
        .child(
            ui::muted(ui::text(text.to_owned(), 12., 19.5))
                .when(title.is_some(), |text| text.mt(px(4.))),
        )
}

/// Read-only YAML with the web editor's 180px minimum viewport.
fn yaml_text(index: usize, text: &SharedString, state: &ArgoTab, live: bool) -> AnyElement {
    div()
        .id(SharedString::from(format!("argo-yaml:{index}")))
        .min_h(px(180.))
        .p(px(12.))
        .border_t_1()
        .border_color(Tokens::border())
        .overflow_x_scroll()
        .font_family(MONO)
        .text_size(px(12.))
        .line_height(px(18.))
        .whitespace_nowrap()
        .test_support()
        .aria_label(text.clone())
        .children(state.editors.get(index).map(|(desired, actual)| {
            gpui_kit::component::input::Editor::new(if live { actual } else { desired })
                .readonly(true)
                .aria_label("Argo CD resource YAML")
                .w_full()
                .h(px(180.))
        }))
        .into_any_element()
}

fn diff_line(line: &DiffLine, highlights: Option<&super::argo_highlight::Highlights>) -> Div {
    let (background, foreground) = controls::diff_colors(line.kind);
    div()
        .h(px(20.))
        .px(px(12.))
        .whitespace_nowrap()
        .bg(background)
        .text_color(foreground)
        .when(line.kind == DiffLineKind::Header, |line| {
            line.font_weight(FontWeight::SEMIBOLD)
        })
        .child(
            gpui_kit::StyledText::new(line.text.clone())
                .with_highlights(highlights.cloned().unwrap_or_default()),
        )
}

/// `ArgoResourceDiff.svelte` in its `overflow-x-auto border-t font-mono
/// text-[0.6875rem] leading-5` wrapper.
fn resource_diff(
    index: usize,
    lines: &[DiffLine],
    full: bool,
    state: &ArgoTab,
    cx: &mut Context<Inspector>,
) -> Div {
    let shown = if full {
        lines
    } else {
        &lines[..lines.len().min(DIFF_PREVIEW)]
    };
    let hidden_changed = lines
        .iter()
        .skip(DIFF_PREVIEW)
        .filter(|line| matches!(line.kind, DiffLineKind::Add | DiffLineKind::Remove))
        .count();
    let body = div()
        .id(SharedString::from(format!("argo-diff:{index}")))
        .test_support()
        .aria_label(
            lines
                .iter()
                .map(|line| line.text.as_ref())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .border_t_1()
        .border_color(Tokens::border())
        .font_family(MONO)
        .text_size(px(11.))
        .line_height(px(20.))
        .children(shown.iter().enumerate().map(|(i, line)| {
            diff_line(
                line,
                state.highlights.get(index).and_then(|lines| lines.get(i)),
            )
        }));
    if lines.len() <= DIFF_PREVIEW {
        return div().child(body);
    }
    div().child(body).child(
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .font_family(crate::theme::SANS)
            .when(!full, |row| {
                row.child(
                    ui::muted(ui::xs(format!(
                        "{} more lines ({hidden_changed} changed) hidden.",
                        lines.len() - DIFF_PREVIEW
                    )))
                    .px(px(12.)),
                )
            })
            .child(
                dui::sm_button(
                    Variant::Ghost,
                    &if full {
                        "Show fewer lines".to_owned()
                    } else {
                        format!("Show all {} lines", lines.len())
                    },
                )
                .id(SharedString::from(format!("argo-diff-lines:{index}")))
                .test_support()
                .on_click(cx.listener(move |this, _, _, cx| {
                    update(this, cx, |state| toggle(&mut state.full, index))
                })),
            ),
    )
}

/// The article body below its header, by `diffView` (the `{#if}` chain).
fn article_body(
    view: &ArgoView,
    state: &ArgoTab,
    index: usize,
    change: &Change,
    cx: &mut Context<Inspector>,
) -> Option<AnyElement> {
    let removal = change.badge == ChangeBadge::Remove;
    let expanded = state.expanded.contains(&index);
    if removal && state.view != DiffView::Target && !expanded {
        return None;
    }
    Some(match (state.view, change.diff.as_deref()) {
        (DiffView::Target, _) if removal => note(
            Some("No desired manifest"),
            "Selected revision no longer contains this resource.",
        )
        .into_any_element(),
        (DiffView::Changes, Some(lines)) => {
            resource_diff(index, lines, state.full.contains(&index), state, cx).into_any_element()
        }
        (DiffView::Changes, None) if removal && !change.live.is_empty() => {
            yaml_text(index, &change.live, state, true)
        }
        (DiffView::Target, _) if !change.desired.is_empty() => yaml_text(index, &change.desired, state, false),
        (DiffView::Live, _) if !change.live.is_empty() => yaml_text(index, &change.live, state, true),
        (current, _) => note(
            None,
            &if change.inexact || view.kubernetes {
                "Exact target and live manifests are unavailable through Kubernetes fallback. Select a connected Argo CD profile for manifest comparison.".to_owned()
            } else {
                format!(
                    "No {} manifest reported.",
                    match current {
                        DiffView::Target => "desired",
                        DiffView::Live => "live",
                        DiffView::Changes => "comparison",
                    }
                )
            },
        )
        .into_any_element(),
    })
}

fn change_badge(badge: ChangeBadge) -> Div {
    match badge {
        ChangeBadge::Remove => ui::status_badge(Tone::Error, "Remove on sync"),
        ChangeBadge::Added => dui::badge(
            (
                Palette::emerald500(0.35),
                Palette::emerald500(0.1),
                Palette::emerald300(),
            ),
            "Added",
        ),
        ChangeBadge::Modified => dui::tinted_badge(Tone::Warning, "Modified"),
        ChangeBadge::InSync => ui::status_badge(Tone::Neutral, "In sync"),
    }
}

/// `<article class="min-w-0 overflow-hidden rounded-lg border bg-background">`.
fn article(
    view: &ArgoView,
    state: &ArgoTab,
    index: usize,
    cx: &mut Context<Inspector>,
) -> impl IntoElement {
    let change = &view.changes[index];
    let removal = change.badge == ChangeBadge::Remove;
    let expanded = state.expanded.contains(&index);
    let header = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .border_b_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .child(
            ui::semibold(ui::xs(change.label.clone()))
                .flex_1()
                .min_w_0()
                .truncate(),
        )
        .child(change_badge(change.badge))
        .when(removal, |header| {
            header.child(
                xs_button(
                    Variant::Ghost,
                    if expanded { "Hide diff" } else { "Show diff" },
                )
                .id(SharedString::from(format!("argo-removal:{}", change.key)))
                .test_support()
                .on_click(cx.listener(move |this, _, _, cx| {
                    update(this, cx, |state| toggle(&mut state.expanded, index))
                })),
            )
        })
        .when(state.focus.is_none(), |header| {
            header.child(
                xs_button(Variant::Ghost, "Focus")
                    .id(SharedString::from(format!("argo-focus:{}", change.key)))
                    .test_support()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.focus_argo_resource(Some(index), window, cx)
                    })),
            )
        });
    div()
        .id(SharedString::from(format!("argo-article:{}", change.key)))
        .test_support()
        .aria_label(change.label.clone())
        .min_w_0()
        .overflow_hidden()
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background())
        .child(header)
        .when(removal, |article| {
            article.child(
                div()
                    .p(px(12.))
                    .bg(Tokens::destructive().opacity(0.1))
                    .child(
                        ui::medium(ui::text("Live-only resource", 12., 19.5))
                            .text_color(Tokens::destructive()),
                    )
                    .child(
                        ui::muted(ui::text(
                            "This resource is no longer present in desired revision. Sync with prune removes live object. Live YAML stays collapsed until Show diff is selected.",
                            12.,
                            19.5,
                        ))
                        .mt(px(4.)),
                    ),
            )
        })
        .children(article_body(view, state, index, change, cx))
}

/// The snapshot `<section>`; `view` is absent while loading or after a failure.
pub(super) fn snapshot(
    view: Option<&ArgoView>,
    state: &ArgoTab,
    failed: bool,
    loading: Phase,
    cx: &mut Context<Inspector>,
) -> Div {
    let focused = state
        .focus
        .and_then(|index| view.and_then(|view| view.changes.get(index)));
    let detail = match (focused, view) {
        (Some(change), _) => change.label.clone(),
        (None, view) => {
            let count = view.map_or(0, |view| view.changes.len());
            format!(
                "{count} reconciliation {} in this snapshot.",
                if count == 1 { "item" } else { "items" }
            )
        }
    };
    let freshness = view.map_or(
        "Kubernetes snapshot loaded when this view opened.",
        |view| view.freshness,
    );
    let heading = div()
        .flex()
        .flex_wrap()
        .items_start()
        .justify_between()
        .gap(px(8.))
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::icon(
                            "git-compare-arrows",
                            14.,
                            Tokens::muted_foreground(),
                        ))
                        .child(caps(if focused.is_some() {
                            "Focused reconciliation"
                        } else {
                            "Application snapshot"
                        })),
                )
                .child(ui::muted(ui::xs(format!("{detail} {freshness}"))).mt(px(4.))),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .when(focused.is_some(), |row| {
                    row.child(
                        xs_button(Variant::Outline, "All changes")
                            .id("argo-snapshot-all")
                            .test_support()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.focus_argo_resource(None, window, cx)
                            })),
                    )
                })
                .child(segmented(state.view, cx)),
        );
    let body = div().mt(px(12.)).flex().flex_col().gap(px(16.));
    let body = match view {
        // ArgoApplicationDetails.svelte checks the loading state first.
        _ if loading.shown() => body.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .p(px(12.))
                .rounded(px(10.))
                .border_1()
                .border_color(Tokens::border())
                .child(ui::icon("loader-circle", 16., Tokens::muted_foreground()))
                .child(ui::muted(ui::xs("Loading Application snapshot"))),
        ),
        _ if !loading.ready() => body,
        None if failed => body.child(dui::empty(
            "argo-snapshot-unavailable",
            "Application snapshot unavailable",
            "Retry Application data before evaluating reconciliation state.",
            None,
        )),
        None => body,
        Some(view) => match state.focus.filter(|index| *index < view.changes.len()) {
            Some(index) => body.child(article(view, state, index, cx)),
            None => {
                let articles: Vec<_> = (0..view.changes.len())
                    .map(|index| article(view, state, index, cx).into_any_element())
                    .collect();
                body.children(articles)
            }
        },
    };
    panel().child(heading).child(body)
}
