//! The grouped list in `GitOpsList.svelte`, separate from the cards.
use super::{actions, name, source_icon, status_tone, tone};
use crate::inspector::{
    Inspector,
    delivery_ui::{self as dui, CARD_GAP},
    gitops_tooltip, surface,
};
use crate::{
    gitops_surface::{
        Selection,
        list::{self as model, Group, SyncTone},
        status_label,
    },
    targets::TestSupportExt,
    theme::{Palette, Tokens},
    ui,
};
use gpui_kit::{Context, Div, Hsla, ScrollHandle, SharedString, div, prelude::*, px};
use kubecove_lib::models::{HealthAssessmentCompleteness, HealthAssessmentState};

/// The list's columns: `minmax(10.5rem,0.9fr) 7.5rem 5rem minmax(7.5rem,2.2fr)
/// 4.5rem minmax(5.5rem,0.8fr) 2.5rem 4rem` with `gap-3 px-3`. Flexible tracks
/// below their minimum freeze there.
const TRACKS: [(f32, f32); 8] = [
    (168., 0.9),
    (120., 0.),
    (80., 0.),
    (120., 2.2),
    (72., 0.),
    (88., 0.8),
    (40., 0.),
    (64., 0.),
];
/// `min-w-[53.75rem]`: the track minimums with their gaps and padding.
const MIN_WIDTH: f32 = 860.;
/// `@min-[68rem]`: Actions widens to `9rem` and shows the Resources button.
const WIDE: f32 = 1088.;
/// `h-9`: the grid line of a row. An expanded row adds a second line below it.
const LINE: f32 = 36.;

/// The track widths, and whether the list is wide enough for Resources.
fn columns(width: f32) -> ([f32; 8], bool) {
    let wide = width >= WIDE;
    let mut tracks = TRACKS;
    if wide {
        tracks[7].0 = 144.;
    }
    (dui::tracks(tracks, MIN_WIDTH, width), wide)
}

/// The list header row: `bg-muted/30 px-3 py-2 text-[0.65rem] uppercase`.
/// `first` is the list's first row, which names the Sync and Destination columns.
pub(in crate::inspector) fn list_header(
    first: Option<Selection>,
    width: f32,
    scroll: &ScrollHandle,
) -> Div {
    let (widths, _) = columns(width);
    let (sync, destination) = model::columns(first);
    let labels = [
        "Name",
        "Health",
        sync,
        "Source(s)",
        "Revision",
        destination,
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
            MIN_WIDTH,
            scroll,
            "gitops-columns".into(),
        ))
        .child(
            ui::semibold(ui::muted(ui::text("ACTIONS", 10.4, 14.)))
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .w(px(widths[7]))
                .flex()
                .items_center()
                .justify_end()
                .pr(px(12.))
                .bg(Tokens::muted()),
        )
}

/// A group header: `bg-muted/30 px-3 py-1.5 text-xs font-semibold` with a
/// chevron and a count chip. A click collapses or expands the group's rows.
pub(in crate::inspector) fn list_group(
    group: Group,
    position: usize,
    count: usize,
    collapsed: bool,
    cx: &mut Context<Inspector>,
) -> Div {
    let chevron = if collapsed {
        "chevron-right"
    } else {
        "chevron-down"
    };
    div().child(
        div()
            .id(SharedString::from(format!("gitops-group:{position}")))
            .test_support()
            .aria_label(format!("{} {count}", group.label()))
            .aria_expanded(!collapsed)
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(6.))
            .bg(Tokens::muted().opacity(0.3))
            .cursor_pointer()
            .child(ui::icon(chevron, 14., Tokens::muted_foreground()))
            .child(ui::semibold(ui::xs(group.label())))
            .child(
                ui::xs(count.to_string())
                    .line_height(px(12.))
                    .px(px(6.))
                    .py(px(2.))
                    .rounded(px(4.))
                    .bg(Tokens::muted()),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_gitops_group(group, cx))),
    )
}

/// The Health cell's icon, its color, and the label's color when it is toned.
fn health_look(state: HealthAssessmentState) -> (&'static str, Hsla, Option<Hsla>) {
    match state {
        HealthAssessmentState::Healthy => ("circle-check", Palette::emerald300(), None),
        HealthAssessmentState::NeedsAttention => (
            "triangle-alert",
            Palette::amber300(),
            Some(Palette::amber300()),
        ),
        HealthAssessmentState::Degraded => (
            "circle-x",
            Tokens::destructive(),
            Some(Tokens::destructive()),
        ),
        HealthAssessmentState::Unknown => ("circle-help", Tokens::muted_foreground(), None),
        HealthAssessmentState::NotEvaluated => ("circle-dashed", Tokens::muted_foreground(), None),
    }
}

/// `text-xs` in a tone, medium unless the tone is quiet.
fn toned(label: String, color: Option<Hsla>) -> Div {
    match color {
        Some(color) => ui::medium(ui::xs(label)).text_color(color),
        None => ui::muted(ui::xs(label)),
    }
}

/// One list row, clipped to the table; the Actions cell stays pinned right
/// as the web's `sticky right-0` does before any horizontal scroll.
pub(in crate::inspector) fn list_row(
    selection: Selection,
    expanded: bool,
    width: f32,
    scroll: &ScrollHandle,
    cx: &mut Context<Inspector>,
) -> Div {
    let (widths, wide) = columns(width - 2.);
    let cell = |index: usize| div().flex_none().w(px(widths[index])).min_w_0();
    let key = selection.key();
    let health = selection.health();
    let (health_icon, health_color, health_text) = health_look(health.state);
    let health_label = match health.completeness {
        HealthAssessmentCompleteness::Partial => {
            format!("{} (partial)", surface::health_state_label(health.state))
        }
        _ => surface::health_state_label(health.state).to_owned(),
    };
    let (sync, sync_tone) = model::sync(selection);
    let sync_color = match sync_tone {
        SyncTone::Quiet => None,
        SyncTone::Active => Some(Tokens::foreground()),
        SyncTone::Attention => Some(Palette::amber300()),
        SyncTone::Failed => Some(Tokens::destructive()),
    };
    let source = selection.source_mode();
    let line = selection.source_line();
    let grid = div()
        .flex()
        .items_center()
        .gap(px(CARD_GAP))
        .h(px(LINE))
        .px(px(12.))
        .child(
            cell(0)
                .flex()
                .items_center()
                .gap(px(6.))
                .pl(px(4.))
                // The name keeps its width and the subtitle gives way first. The web
                // list name's `onActivate` opens the selection, as a card click does.
                .child({
                    let key = key.clone();
                    div()
                        .id(SharedString::from(format!("gitops-list-name:{key}")))
                        .flex_none()
                        .max_w_full()
                        .min_w_0()
                        .cursor_pointer()
                        .child(name(selection, 14., 20., cx))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_gitops_selection(&key, window, cx)
                        }))
                })
                .child(
                    ui::muted(ui::xs(selection.subtitle().to_owned()))
                        .min_w_0()
                        .truncate(),
                ),
        )
        .child(gitops_tooltip::hint(
            cell(1)
                .flex()
                .items_center()
                .gap(px(6.))
                .child(ui::icon(health_icon, 14., health_color))
                .child(toned(health_label.clone(), health_text).truncate()),
            format!("gitops-list-health:{key}"),
            gitops_tooltip::health(selection, &health_label),
        ))
        .child(cell(2).child(toned(sync, sync_color).truncate()))
        .child(match (source, line) {
            (None, None) => cell(3).child(ui::muted(ui::sm("-"))).into_any_element(),
            (source, line) => gitops_tooltip::reveal(
                cell(3)
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .when_some(source, |cell, mode| {
                        let (icon, _, color) = source_icon(mode);
                        cell.child(ui::icon(icon, 14., color))
                    })
                    .child(
                        ui::muted(ui::xs(line.unwrap_or_else(|| "-".into())))
                            .flex_1()
                            .min_w_0()
                            .truncate(),
                    ),
                format!("gitops-source:{key}"),
                gitops_tooltip::source(selection),
                "source details".to_owned(),
                cx,
            )
            .into_any_element(),
        })
        .child(gitops_tooltip::reveal(
            cell(4).child(ui::sm(selection.revision_label().unwrap_or_default()).truncate()),
            format!("gitops-list-revision:{key}"),
            gitops_tooltip::revision(selection),
            "revision".to_owned(),
            cx,
        ))
        .child(cell(5).child(ui::muted(ui::sm(selection.destination().to_owned())).truncate()))
        .child(cell(6).child(ui::muted(ui::sm(selection.age().to_owned()))));
    let body = div()
        .relative()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(Tokens::card())
        .when(
            matches!(
                health.state,
                HealthAssessmentState::Degraded | HealthAssessmentState::NeedsAttention
            ),
            |row| {
                row.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(4.))
                        .bg(tone(selection)),
                )
            },
        )
        .child(grid)
        .when(expanded, |row| {
            // `flex flex-wrap gap-1.5 pb-2 pl-4 pr-3 text-xs text-muted-foreground`.
            row.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .pb(px(8.))
                    .pl(px(16.))
                    .pr(px(12.))
                    .when(!health.reasons.is_empty(), |line| {
                        line.child(ui::muted(ui::xs(health.reasons.join(" · "))).mr(px(4.)))
                    })
                    .children(selection.badges().into_iter().map(|(label, value)| {
                        dui::tinted_badge(
                            status_tone(value),
                            format!("{label}: {}", status_label(value)),
                        )
                    })),
            )
        });
    div()
        .relative()
        .child(dui::horizontal(
            body,
            width,
            MIN_WIDTH,
            scroll,
            format!("gitops-columns:{key}"),
        ))
        .child(
            div()
                .absolute()
                .right_0()
                .top_0()
                .h(px(LINE))
                .w(px(widths[7]))
                .pr(px(12.))
                .flex()
                .items_center()
                .justify_end()
                .bg(Tokens::card())
                .child(actions(selection, wide.then_some("Resources"), cx).gap(px(4.))),
        )
}
