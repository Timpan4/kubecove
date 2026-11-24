//! The "Application snapshot" section of `ArgoApplicationDetails.svelte`: one
//! `<article>` per reconciliation resource. `body` renders what each article
//! shows below its header.

use super::super::Inspector;
use super::super::controls::{self, Variant};
use super::super::delivery_ui as dui;
use super::super::loading::Phase;
use super::panels::{caps, panel};
use super::tab::ArgoTab;
use crate::argo_app::{ArgoView, ChangeBadge};
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui::{self, Tone};
use body::article_body;
use gpui_kit::{Context, Div, SharedString, div, prelude::*, px};

mod body;

pub(super) use body::Body;

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
    controls::button_frame(variant, 20., false)
        .rounded(px(4.))
        .child(variant.fill(4.))
        .text_size(px(10.))
        .line_height(px(15.))
        .child(label.to_owned())
}

/// `SegmentedControl size="sm"` for the reconciliation document view.
fn segmented(active: DiffView, cx: &mut Context<Inspector>) -> Div {
    let slide = crate::control_motion::Slide::new();
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .p(px(2.))
        .rounded(px(10.))
        .bg(Tokens::background())
        .border_1()
        .border_color(Tokens::border().opacity(0.6))
        .child(controls::segment_pill(
            &slide,
            "segment-pill:argo-view",
            VIEWS
                .iter()
                .position(|(option, _)| *option == active)
                .unwrap_or_default(),
        ))
        .children(VIEWS.map(|(option, label)| {
            let segment = div()
                .id(SharedString::from(format!("argo-view:{label}")))
                .test_support()
                .aria_label(label)
                .cursor_pointer()
                .flex()
                .items_center()
                .h(px(24.))
                .px(px(8.))
                .rounded(px(8.))
                .child(ui::medium(ui::xs(label)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    update(this, cx, |state| state.view = option);
                    this.ensure_argo_editors(window, cx);
                }));
            let segment = crate::control_motion::eased(
                SharedString::from(format!("segment:argo-view:{label}")),
                option == active,
                move |on| controls::segment(segment, on),
            );
            slide.option(option == active, segment)
        }))
}

fn change_badge(badge: ChangeBadge) -> Div {
    match badge {
        ChangeBadge::Remove => ui::status_badge(Tone::Error, "Remove on sync"),
        ChangeBadge::Added => ui::badge(
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
                .on_click(cx.listener(move |this, _, window, cx| {
                    update(this, cx, |state| toggle(&mut state.expanded, index));
                    this.ensure_argo_editors(window, cx);
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
    reveal: Option<f32>,
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
    let body = div()
        .mt(px(12.))
        .flex()
        .flex_col()
        .gap(px(16.))
        .when_some(reveal, |body, opacity| body.opacity(opacity));
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
                .child(ui::spinner(16., Tokens::muted_foreground()))
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
