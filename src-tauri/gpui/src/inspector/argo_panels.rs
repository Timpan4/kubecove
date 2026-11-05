//! The Argo CD tab's panels from `ArgoApplicationDetails.svelte`: the change
//! navigator, and the Delivery and Recent deployment cards.

use super::Inspector;
use super::argo_tab::ArgoTab;
use super::controls::{self, Variant};
use crate::argo_app::ArgoView;
use crate::model::exact_timestamp;
use crate::theme::Tokens;
use crate::ui::{self, Tone};
use gpui_kit::TestSupportExt;
use gpui_kit::{Context, Div, SharedString, div, prelude::*, px};

/// `rounded-lg border bg-surface-1 p-3`.
pub(super) fn panel() -> Div {
    div()
        .min_w_0()
        .p(px(12.))
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
}

/// `text-xs font-semibold uppercase tracking-wide text-muted-foreground`.
pub(super) fn caps(label: &str) -> Div {
    ui::semibold(ui::muted(ui::xs(label.to_uppercase())))
}

pub(super) fn navigator(view: &ArgoView, state: &ArgoTab, cx: &mut Context<Inspector>) -> Div {
    let all = controls::button(
        if state.focus.is_none() {
            Variant::Secondary
        } else {
            Variant::Outline
        },
        24.,
        false,
    )
    .id("argo-all-changes")
    .test_support()
    .child("All changes")
    .on_click(cx.listener(|this, _, window, cx| this.focus_argo_resource(None, window, cx)));
    panel()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(caps("Change navigator"))
                        .child(
                            ui::muted(ui::xs(
                                "Focus one reconciliation item or keep the complete snapshot.",
                            ))
                            .mt(px(2.)),
                        ),
                )
                .child(ui::outline_badge(view.changes.len().to_string())),
        )
        .child(
            div()
                .id("argo-navigator")
                .test_support()
                .mt(px(12.))
                .pb(px(4.))
                .flex()
                .gap(px(8.))
                .overflow_x_scroll()
                .restrict_scroll_to_axis()
                .on_scroll(|event, window, cx| {
                    let delta = event.delta.pixel_delta(window.line_height());
                    // GPUI otherwise remaps the horizontal delta onto the detail's y axis.
                    if delta.x.abs() > delta.y.abs() {
                        cx.stop_propagation();
                    }
                })
                .child(all)
                .children(view.changes.iter().enumerate().map(|(index, change)| {
                    let selected = state.focus == Some(index);
                    controls::button(
                        if selected {
                            Variant::Secondary
                        } else {
                            Variant::Outline
                        },
                        24.,
                        false,
                    )
                    .id(SharedString::from(format!("argo-change:{}", change.key)))
                    .test_support()
                    .h_auto()
                    .min_w(px(144.))
                    .max_w(px(224.))
                    .p(px(8.))
                    .justify_start()
                    .child(
                        div()
                            .min_w_0()
                            .child(ui::medium(ui::xs(change.chip.clone())).truncate())
                            .child(
                                div()
                                    .mt(px(2.))
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(4.))
                                    .child(ui::outline_badge(change.status.clone()))
                                    .when(change.prune, |row| {
                                        row.child(ui::status_badge(Tone::Error, "Prune"))
                                    }),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.focus_argo_resource(Some(index), window, cx)
                    }))
                })),
        )
}

pub(super) fn delivery(
    view: &ArgoView,
    state: &ArgoTab,
    timezone: crate::preferences::TimestampTimezone,
) -> Div {
    let facts = [
        ("Repository", &view.repository),
        ("Path", &view.path),
        ("Revision", &view.configured_revision),
        ("Destination", &view.destination),
        ("Policy", &view.policy),
    ];
    let deployment = state
        .options
        .history
        .and_then(|id| {
            state
                .inspector
                .as_ref()?
                .history
                .iter()
                .position(|h| h.id == Some(id))
        })
        .and_then(|index| view.history.get(index))
        .or_else(|| view.history.first());
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            panel().child(caps("Delivery")).child(
                div()
                    .mt(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(facts.map(|(label, value)| {
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(ui::muted(ui::xs(label)).w(px(72.)).flex_none())
                            .child(ui::xs(value.clone()).flex_1().min_w_0())
                    })),
            ),
        )
        .child(
            panel()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::icon(
                            "git-commit-horizontal",
                            14.,
                            Tokens::muted_foreground(),
                        ))
                        .child(caps("Recent deployment")),
                )
                .when(view.history.len() > 1, |card| {
                    card.child(
                        gpui_kit::component::select::Select::new(&state.options.picker)
                            .id("argo-history-picker")
                            .h(px(28.))
                            .flex_none()
                            .w_full()
                            .mt(px(12.)),
                    )
                })
                .child(match deployment {
                    Some(entry) => div()
                        .flex_none()
                        .id("argo-history-summary")
                        .test_support()
                        .aria_label(format!("{} {}", entry.revision, entry.initiated_by))
                        .child(ui::semibold(ui::sm(entry.revision.clone())).mt(px(12.)))
                        .child(
                            ui::muted(ui::text(
                                format!(
                                    "{} by {}",
                                    entry
                                        .deployed_at
                                        .as_deref()
                                        .and_then(|value| exact_timestamp(value, timezone))
                                        .unwrap_or_else(|| "No deployment timestamp".into()),
                                    entry.initiated_by
                                ),
                                12.,
                                19.5,
                            ))
                            .mt(px(4.)),
                        )
                        .into_any_element(),
                    None => div()
                        .child(ui::muted(ui::xs("No deployment history reported.")).mt(px(12.)))
                        .into_any_element(),
                }),
        )
}
