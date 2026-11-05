//! The Argo CD tab's panels from `ArgoApplicationDetails.svelte`: the change
//! navigator, and the Delivery and Recent deployment cards.

use super::argo_tab::ArgoTab;
use super::controls::{self, Variant};
use super::{Inspector, surface};
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
    .on_click(cx.listener(|this, _, _, cx| {
        if let Some(state) = this.argo_tab_state() {
            state.focus = None;
            cx.notify();
        }
    }));
    panel()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    div().child(caps("Change navigator")).child(
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
                .mt(px(12.))
                .pb(px(4.))
                .flex()
                .gap(px(8.))
                .overflow_x_scroll()
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
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(state) = this.argo_tab_state() {
                            state.focus = Some(index);
                            cx.notify();
                        }
                    }))
                })),
        )
}

pub(super) fn delivery(view: &ArgoView, timezone: crate::preferences::TimestampTimezone) -> Div {
    let facts = [
        ("Repository", &view.repository),
        ("Path", &view.path),
        ("Revision", &view.configured_revision),
        ("Destination", &view.destination),
        ("Policy", &view.policy),
    ];
    let deployment = view.history.first();
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
                    card.child(surface::disabled(
                        div()
                            .mt(px(12.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .h(px(28.))
                            .px(px(10.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(Tokens::input())
                            .child(ui::xs(
                                deployment.map_or(String::new(), |entry| entry.revision.clone()),
                            ))
                            .child(ui::icon("chevron-down", 14., Tokens::muted_foreground())),
                    ))
                })
                .child(match deployment {
                    Some(entry) => div()
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
                        ),
                    None => div()
                        .child(ui::muted(ui::xs("No deployment history reported.")).mt(px(12.))),
                }),
        )
}
