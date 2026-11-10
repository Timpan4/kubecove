//! The RBAC Cockpit's category and risk toolbar and its review queue, from
//! `RbacView.svelte`, with the risk and review badges they share with the
//! inspector.

use super::super::controls::{Variant, button};
use super::super::{Inspector, surface};
use super::page::PAGE;
use crate::rbac::review::{self, ReviewRecord, Status};
use crate::rbac::{self, Bucket, CockpitItem, RbacView};
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::component::input::Input;
use gpui_kit::{AnyElement, Context, Div, Hsla, SharedString, div, prelude::*, px};
use kubecove_lib::models::{RbacInspectionSummary, RbacRiskIndicator, RbacRiskLevel};

impl Inspector {
    pub(super) fn rbac_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.rbac;
        let data = state.data().expect("ready");
        let categories = RbacView::ALL.into_iter().map(|view| {
            let selected = view == state.view;
            button(
                if selected {
                    Variant::Secondary
                } else {
                    Variant::Ghost
                },
                24.,
                false,
            )
            .id(SharedString::from(format!(
                "rbac-category:{}",
                view.label()
            )))
            .test_support()
            .aria_label(view.label())
            .aria_selected(selected)
            .child(view.label())
            .child(
                ui::outline_badge(view.count(data).to_string())
                    .h(px(16.))
                    .px(px(6.)),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.rbac.kind_location = false;
                this.set_rbac_view(view, window, cx);
            }))
        });
        let risks = Bucket::ALL
            .into_iter()
            .zip(state.counts)
            .map(|((bucket, label), count)| {
                let selected = bucket == state.bucket;
                button(
                    if selected {
                        Variant::Default
                    } else {
                        Variant::Outline
                    },
                    20.,
                    false,
                )
                .id(SharedString::from(format!("rbac-risk-filter:{label}")))
                .test_support()
                .aria_label(label)
                .aria_selected(selected)
                // `size="xs"`: `rounded-sm px-2 text-[0.625rem]`.
                .rounded(px(6.))
                .text_size(px(10.))
                .line_height(px(15.))
                .child(label)
                .child(div().opacity(0.75).child(count.to_string()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.rbac.bucket = bucket;
                    this.rbac.shown = PAGE;
                    this.refilter_rbac(cx);
                }))
            });
        // The `size="sm"` card keeps its `py-3` and its content's `px-3`,
        // because the variant-prefixed classes outrank `py-0` and `px-2`.
        ui::card().py(px(12.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(12.))
                .py(px(8.))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(4.))
                        .child(ui::caps("Explore", 10.).px(px(4.)))
                        .children(categories),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .flex_wrap()
                        .min_w_0()
                        .items_center()
                        .justify_end()
                        .gap(px(4.))
                        .child(ui::caps("Risk", 10.).px(px(4.)))
                        .children(risks)
                        .child(
                            div().flex_1().min_w(px(192.)).max_w(px(288.)).child(
                                Input::new(&state.search)
                                    .id("rbac-search")
                                    .aria_label("Search RBAC objects")
                                    .prefix(ui::icon("search", 14., Tokens::muted_foreground()))
                                    .text_xs()
                                    .h_7(),
                            ),
                        ),
                ),
        )
    }

    pub(super) fn rbac_queue(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.rbac;
        let data = state.data().expect("ready");
        let selected = state.selected_item().map(|item| item.key.as_str());
        let title = if state.view == RbacView::ServiceAccounts {
            "Service account review queue".to_owned()
        } else {
            format!("{} review queue", state.view.label())
        };
        // Bounded like the web: PAGE cards, PAGE more per "Load 50 more".
        let cards: Vec<AnyElement> = state
            .filtered
            .iter()
            .take(state.shown)
            .map(|index| {
                let item = &state.items[*index];
                queue_card(
                    data,
                    &state.reviews,
                    item,
                    selected == Some(item.key.as_str()),
                    cx,
                )
            })
            .collect();
        let empty = cards.is_empty();
        let more = state.filtered.len() > state.shown;
        // `minmax(21rem,2fr)` beside the inspector's `minmax(35rem,3fr)`.
        ui::card()
            .py(px(12.))
            // `data-[size=sm]:gap-3` outranks the view's `gap-0`.
            .gap(px(12.))
            .flex_basis(px(0.))
            .flex_grow(2.)
            .min_w(px(336.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(12.))
                    .px(px(12.))
                    .py(px(12.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(ui::semibold(ui::sm(title)))
                            .child(
                                ui::muted(ui::xs(
                                    "Highest risk · most findings · namespace and name",
                                ))
                                .mt(px(2.)),
                            ),
                    )
                    .child(surface::secondary_badge(state.filtered.len().to_string())),
            )
            .child(
                div()
                    .id("rbac-queue")
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .p(px(8.))
                    .children(cards)
                    .when(empty, |list| {
                        list.child(
                            ui::muted(ui::sm("No matching RBAC objects."))
                                .id("rbac-empty")
                                .test_support()
                                .aria_label("No matching RBAC objects.")
                                .p(px(24.))
                                .text_center(),
                        )
                    })
                    .when(more, |list| {
                        list.child(
                            button(Variant::Outline, 24., false)
                                .id("rbac-more")
                                .test_support()
                                .w_full()
                                .child("Load 50 more")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.rbac.shown += PAGE;
                                    cx.notify();
                                })),
                        )
                    }),
            )
    }
}

pub(super) fn has_level(risks: &[RbacRiskIndicator], level: RbacRiskLevel) -> bool {
    risks.iter().any(|risk| risk.level == level)
}

/// `riskRailClass`.
pub(super) fn risk_rail(risks: &[RbacRiskIndicator]) -> Hsla {
    if has_level(risks, RbacRiskLevel::High) {
        Tokens::destructive()
    } else if has_level(risks, RbacRiskLevel::Medium) {
        Palette::amber500(1.)
    } else if has_level(risks, RbacRiskLevel::Low) {
        Palette::sky500(1.)
    } else if has_level(risks, RbacRiskLevel::Unknown) {
        Tokens::muted_foreground()
    } else {
        Palette::emerald500(1.)
    }
}

pub(super) fn tinted_badge(
    label: impl Into<SharedString>,
    foreground: Hsla,
    border: Hsla,
    background: Hsla,
) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .h(px(20.))
        .px(px(8.))
        .rounded_full()
        .border_1()
        .border_color(border)
        .bg(background)
        .child(
            ui::text(label, 10., 14.)
                .flex_none()
                .whitespace_nowrap()
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(foreground),
        )
}

/// `riskBadgeClass` around `riskSummaryLabel`.
pub(super) fn risk_badge(risks: &[RbacRiskIndicator], label: impl Into<SharedString>) -> Div {
    if has_level(risks, RbacRiskLevel::High) {
        tinted_badge(
            label,
            Tokens::destructive(),
            Tokens::destructive().opacity(0.35),
            Tokens::destructive().opacity(0.1),
        )
    } else if has_level(risks, RbacRiskLevel::Medium) {
        tinted_badge(
            label,
            Palette::amber300(),
            Palette::amber500(0.35),
            Palette::amber500(0.1),
        )
    } else if has_level(risks, RbacRiskLevel::Unknown) {
        tinted_badge(
            label,
            Tokens::muted_foreground(),
            Tokens::muted_foreground().opacity(0.35),
            Tokens::muted(),
        )
    } else {
        ui::outline_badge(label)
    }
}

/// `RbacReviewBadge`, shown for reviewable categories.
pub(super) fn review_badge(
    data: &RbacInspectionSummary,
    records: &[ReviewRecord],
    item: &CockpitItem,
) -> Option<Div> {
    if !item.category.reviewable() {
        return None;
    }
    let record = review::find(records, &data.cluster, &item.key);
    let status = review::status(record, data, item);
    let disposition = record.map(|record| record.disposition);
    Some(match (status, disposition) {
        (Status::Stale, _) => tinted_badge(
            "Review stale",
            Palette::amber300(),
            Palette::amber500(0.35),
            Palette::amber500(0.1),
        ),
        (Status::Active, Some(review::Disposition::Expected)) => tinted_badge(
            "Expected",
            Palette::emerald300(),
            Palette::emerald500(0.35),
            Palette::emerald500(0.1),
        ),
        (Status::Active, Some(review::Disposition::Anomalous)) => tinted_badge(
            "Anomalous",
            Tokens::destructive(),
            Tokens::destructive().opacity(0.35),
            Tokens::destructive().opacity(0.1),
        ),
        _ => tinted_badge(
            "Unreviewed",
            Tokens::muted_foreground(),
            Tokens::border(),
            Tokens::input().opacity(0.3),
        ),
    })
}

fn queue_card(
    data: &RbacInspectionSummary,
    records: &[ReviewRecord],
    item: &CockpitItem,
    selected: bool,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    let title = format!("{}/{}", item.kind, item.name);
    let risk = rbac::risk_summary_label(&item.risks);
    let key = item.key.clone();
    div()
        .id(SharedString::from(format!("rbac-item:{}", item.key)))
        .test_support()
        .aria_label(title.clone())
        .aria_description(risk.clone())
        .aria_selected(selected)
        .relative()
        .w_full()
        .p(px(12.))
        .pl(px(14.))
        .rounded(px(8.))
        .border_1()
        .border_color(if selected {
            Tokens::primary().opacity(0.7)
        } else {
            Tokens::border()
        })
        .bg(if selected {
            Tokens::primary().opacity(0.05)
        } else {
            Tokens::card()
        })
        .overflow_hidden()
        .cursor_pointer()
        .hover(|card| card.bg(Tokens::muted().opacity(0.6)))
        .child(
            surface::left_rule(if selected {
                Tokens::primary().opacity(0.7)
            } else {
                risk_rail(&item.risks)
            })
            .w(px(3.)),
        )
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(8.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(ui::semibold(ui::sm(title)).truncate())
                        .child(
                            ui::muted(ui::xs(
                                item.namespace
                                    .clone()
                                    .unwrap_or_else(|| "cluster scope".into()),
                            ))
                            .truncate(),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .items_end()
                        .gap(px(4.))
                        .child(risk_badge(&item.risks, risk))
                        .children(review_badge(data, records, item)),
                ),
        )
        .child(
            div().flex().flex_wrap().gap(px(4.)).mt(px(8.)).children(
                rbac::queue_tags(data, item)
                    .into_iter()
                    .map(|tag| ui::outline_badge(tag).h(px(16.)).px(px(6.)).rounded(px(4.))),
            ),
        )
        .child(
            ui::muted(ui::xs(rbac::queue_summary(data, item)))
                .mt(px(8.))
                .truncate(),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.select_rbac_item(key.clone(), cx)))
        .into_any_element()
}
