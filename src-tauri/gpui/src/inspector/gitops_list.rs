//! The table rows in `GitOpsView.svelte`, separate from its cards.
use super::{actions, name, source_badge, status_tone, tone};
use crate::inspector::{
    Inspector,
    delivery_ui::{self as dui, CARD_GAP},
    gitops_tooltip, surface,
};
use crate::{
    gitops_surface::{Selection, status_label},
    theme::Tokens,
    ui,
};
use gpui_kit::{Context, Div, ScrollHandle, div, prelude::*, px};

/// The list's columns: `minmax(12rem,1fr) 9rem minmax(16rem,1.35fr)
/// minmax(7rem,0.6fr) minmax(9rem,0.75fr) 4rem 9rem` with `gap-3 px-3`, at
/// least `68rem` wide. Flexible tracks below their minimum freeze there.
fn columns(width: f32) -> [f32; 7] {
    dui::tracks(
        [
            (192., 1.),
            (144., 0.),
            (256., 1.35),
            (112., 0.6),
            (144., 0.75),
            (64., 0.),
            (144., 0.),
        ],
        1088.,
        width,
    )
}

/// The list header row: `bg-muted/30 px-3 py-2 text-[0.65rem] uppercase`.
pub(in crate::inspector) fn list_header(width: f32, scroll: &ScrollHandle) -> Div {
    let widths = columns(width);
    let labels = [
        "Name",
        "Status",
        "Source(s)",
        "Revision",
        "Destination/Namespace",
        "Age",
    ];
    let body = div()
        .relative()
        .flex()
        .gap(px(CARD_GAP))
        .px(px(12.))
        .py(px(8.))
        .overflow_hidden()
        .border_b_1()
        .border_color(Tokens::border())
        .bg(Tokens::muted().opacity(0.3))
        .children(labels.iter().zip(widths).map(|(label, width)| {
            ui::semibold(ui::muted(ui::text(label.to_uppercase(), 10.4, 14.)))
                .flex_none()
                .w(px(width))
                .whitespace_nowrap()
        }));
    div()
        .relative()
        .child(dui::horizontal(
            body,
            width,
            1088.,
            scroll,
            "gitops-columns".into(),
        ))
        .child(
            ui::semibold(ui::muted(ui::text("ACTIONS", 10.4, 14.)))
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .w(px(widths[6]))
                .flex()
                .items_center()
                .justify_end()
                .pr(px(12.))
                .bg(Tokens::muted()),
        )
}

/// One list row, clipped to the table; the Actions cell stays pinned right
/// as the web's `sticky right-0` does before any horizontal scroll.
pub(in crate::inspector) fn list_row(
    selection: Selection,
    width: f32,
    scroll: &ScrollHandle,
    cx: &mut Context<Inspector>,
) -> Div {
    let widths = columns(width - 2.);
    let cell = |index: usize| div().flex_none().w(px(widths[index])).min_w_0();
    let source = selection.source_mode();
    let line = selection.source_line();
    let body = div()
        .relative()
        .flex()
        .items_center()
        .gap(px(CARD_GAP))
        .px(px(12.))
        .py(px(10.))
        .overflow_hidden()
        .bg(Tokens::card())
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(4.))
                .bg(tone(selection)),
        )
        .child(
            cell(0)
                .pl(px(4.))
                .child(name(selection, 14., 20., cx))
                .child(ui::muted(ui::xs(selection.subtitle().to_owned())).truncate()),
        )
        .child(
            cell(1)
                .flex()
                .flex_col()
                .items_start()
                .gap(px(4.))
                .child(surface::health_badge(selection.health()))
                .children(selection.badges().into_iter().map(|(label, value)| {
                    dui::tinted_badge(
                        status_tone(value),
                        format!("Raw {label}: {}", status_label(value)),
                    )
                })),
        )
        .child(match (source, line) {
            (None, None) => cell(2).child(ui::muted(ui::sm("-"))),
            (source, line) => cell(2)
                .flex()
                .items_center()
                .gap(px(8.))
                .when_some(source, |cell, mode| {
                    cell.child(source_badge(selection, mode, cx))
                })
                .child(
                    ui::muted(ui::xs(line.unwrap_or_else(|| "-".into())))
                        .flex_1()
                        .min_w_0()
                        .truncate(),
                ),
        })
        .child(gitops_tooltip::reveal(
            cell(3).child(
                ui::medium(ui::sm(selection.revision_label().unwrap_or_default())).truncate(),
            ),
            format!("gitops-list-revision:{}", selection.key()),
            gitops_tooltip::revision(selection),
            cx,
        ))
        .child(cell(4).child(ui::muted(ui::sm(selection.destination().to_owned())).truncate()))
        .child(cell(5).child(ui::muted(ui::sm(selection.age().to_owned()))));
    div()
        .relative()
        .child(dui::horizontal(
            body,
            width,
            1088.,
            scroll,
            format!("gitops-columns:{}", selection.key()),
        ))
        .child(
            div()
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .w(px(widths[6]))
                .pr(px(12.))
                .flex()
                .items_center()
                .justify_end()
                .bg(Tokens::card())
                .child(actions(selection, "Resources", cx).gap(px(4.))),
        )
}
