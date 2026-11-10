//! The provider summary card the GitOps and Helm pages open with
//! (`GitOpsView.svelte` and `HelmView.svelte`).

use super::delivery_ui::badge;
use crate::targets::TestSupportExt;
use crate::theme::{HEADING, Palette, Tokens};
use crate::ui;
use gpui_kit::{Div, FontWeight, Hsla, SharedString, div, prelude::*, px};

/// One summary fact: a `text-[0.62rem]` caps label over a semibold value.
pub(super) struct Fact {
    pub label: &'static str,
    pub value: usize,
    pub color: Hsla,
}

/// The detected badges under a summary title.
pub(super) enum Badges {
    /// GitOps: `mt-2 flex flex-wrap gap-1.5`.
    Row(Vec<String>),
    /// Helm: one badge inside a block `mt-2` div. Its line box (the body's
    /// 24px strut, baseline-aligned with the badge text) pushes the 20px
    /// badge 4.5px down and makes the box 24.5px tall.
    Line(Option<String>),
}

/// `section.rounded-lg border bg-surface-1 px-3 py-2.5 shadow-sm`: the
/// provider title, detected badges, the object total and the fact grid.
pub(super) fn summary(
    id: &str,
    title: &str,
    badges: Badges,
    total: (usize, &str),
    facts: &[Fact],
    columns: usize,
) -> Div {
    let grid = facts.chunks(columns).map(|row| {
        div()
            .flex()
            .gap(px(16.))
            .children(row.iter().map(|fact| {
                div()
                    .id(SharedString::from(format!("{id}-fact:{}", fact.label)))
                    .test_support()
                    .aria_label(fact.value.to_string())
                    .flex_1()
                    .flex_basis(px(0.))
                    .min_w_0()
                    .child(
                        // `text-[0.62rem]` keeps the body's 1.5 line height.
                        ui::text(fact.label.to_uppercase(), 9.92, 14.88)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Tokens::muted_foreground()),
                    )
                    .child(
                        // No size class: the body's 16px/24px.
                        ui::semibold(ui::text(fact.value.to_string(), 16., 24.))
                            .mt(px(2.))
                            .text_color(fact.color),
                    )
            }))
            .children((row.len()..columns).map(|_| div().flex_1().flex_basis(px(0.))))
    });
    div()
        .px(px(12.))
        .py(px(10.))
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .shadow_sm()
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .min_w_0()
                        .child(
                            ui::text(title.to_owned(), 16., 24.)
                                .font_family(HEADING)
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                        .map(|column| {
                            let detected = |label| {
                                badge(
                                    (
                                        Palette::emerald500(0.35),
                                        Palette::emerald500(0.1),
                                        Palette::emerald300(),
                                    ),
                                    label,
                                )
                            };
                            match badges {
                                Badges::Row(labels) if !labels.is_empty() => column.child(
                                    div()
                                        .mt(px(8.))
                                        .flex()
                                        .flex_wrap()
                                        .gap(px(6.))
                                        .children(labels.into_iter().map(detected)),
                                ),
                                Badges::Line(Some(label)) => column.child(
                                    div().mt(px(8.)).pt(px(4.5)).flex().child(detected(label)),
                                ),
                                _ => column,
                            }
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .child(ui::semibold(ui::text(total.0.to_string(), 16., 24.)))
                        .child(ui::muted(ui::xs(total.1.to_owned()))),
                ),
        )
        .child(
            div()
                .mt(px(10.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(grid),
        )
}
