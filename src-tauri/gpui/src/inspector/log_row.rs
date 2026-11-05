//! One row of the Logs tab, mirroring the default stacked layout of
//! `LogEntry.svelte`: time, level badge, headline, source, error and fields.

use super::Inspector;
use super::controls::{Variant, button, red300};
use super::events::tw;
use super::logs_state::LogsState;
use crate::logs::{LogLevel, ParsedLogLine, highlight_ranges};
use crate::theme::{MONO, Tokens, oklch};
use crate::ui;
use gpui_kit::base::ObservedElement;
use gpui_kit::{
    ClipboardItem, Context, Div, FontWeight, HighlightStyle, Hsla, SharedString, Stateful,
    StyledText, TestSupportExt, div, prelude::*, px, relative,
};

fn red400(alpha: f32) -> Hsla {
    oklch(0.704, 0.191, 22.216, alpha)
}
pub(super) fn red500(alpha: f32) -> Hsla {
    tw::red500(alpha)
}
fn yellow300(alpha: f32) -> Hsla {
    oklch(0.905, 0.182, 98.111, alpha)
}

/// `levelChips` active classes: (border, background, foreground).
pub(super) fn chip_colors(level: LogLevel) -> (Hsla, Hsla, Hsla) {
    match level {
        LogLevel::Error => (red500(0.45), red500(0.1), red400(1.)),
        LogLevel::Warn => (tw::amber500(0.45), tw::amber500(0.1), tw::amber400(1.)),
        LogLevel::Info => (tw::sky500(0.45), tw::sky500(0.1), tw::sky300()),
        LogLevel::Debug => (
            Tokens::border(),
            Tokens::muted(),
            Tokens::foreground().opacity(0.8),
        ),
    }
}

/// `LEVEL_BADGE_CLASSES`: (background, foreground).
fn badge_colors(level: LogLevel) -> (Hsla, Hsla) {
    let clear = gpui_kit::transparent_black();
    match level {
        LogLevel::Error => (red400(1.), oklch(0.258, 0.092, 26.042, 1.)),
        LogLevel::Warn => (tw::amber500(0.15), tw::amber400(1.)),
        LogLevel::Info => (clear, tw::sky300()),
        LogLevel::Debug => (clear, Tokens::muted_foreground()),
    }
}

/// `highlighted`: filter matches marked `bg-yellow-300/40`.
fn highlighted(text: &SharedString, term: &str) -> StyledText {
    let style = HighlightStyle {
        background_color: Some(yellow300(0.4)),
        ..Default::default()
    };
    let ranges = highlight_ranges(text, term);
    StyledText::new(text.clone()).with_highlights(ranges.into_iter().map(|range| (range, style)))
}

/// `fieldChips` item: a muted key and its value, both marked for the filter.
fn field_pair(key: &SharedString, value: &SharedString, term: &str) -> Div {
    div()
        .flex()
        .child(
            div()
                .text_color(Tokens::muted_foreground())
                .child(highlighted(key, term)),
        )
        .child(div().w(px(6.3)))
        .child(highlighted(value, term))
}

/// `LogEntry.svelte`'s `oneLine` row. The summary line toggles the row. An
/// open row drops the inline error and fields for a block with the error,
/// the field chips and Copy line.
fn one_line_row(
    line: &ParsedLogLine,
    logs: &LogsState,
    position: usize,
    cx: &mut Context<Inspector>,
) -> ObservedElement<Stateful<Div>> {
    let structure = &line.structure;
    let index = line.index;
    let open = logs.row_open(index);
    let term = logs.term.as_str();
    let summary = div()
        .id(("log-summary", position))
        .test_support()
        .flex()
        .gap(px(8.))
        .min_w_0()
        .font_family(MONO)
        .text_size(px(12.))
        .child(line.time.clone())
        .children(structure.level.map(|level| ui::xs(level.label())))
        .children(
            (logs.is_aggregate() && !line.source_label.is_empty())
                .then(|| ui::xs(line.source_label.clone())),
        )
        .child(highlighted(&structure.headline, term))
        .when(!open, |summary| {
            summary
                .children(
                    structure
                        .error
                        .as_ref()
                        .map(|error| highlighted(error, term)),
                )
                .children(structure.fields.iter().map(|field| {
                    highlighted(&format!("{}={}", field.key, field.value).into(), term)
                }))
        });
    let details = open.then(|| {
        div()
            .id(("log-details", position))
            .test_support()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(4.))
            .pt(px(2.))
            .pb(px(4.))
            .children(structure.error.as_ref().map(|error| {
                div()
                    .font_family(MONO)
                    .text_size(px(11.5))
                    .line_height(px(16.))
                    .text_color(red300())
                    .child(highlighted(error, term))
            }))
            .when(!structure.fields.is_empty(), |details| {
                details.child(div().flex().flex_wrap().gap(px(3.)).children(
                    structure.fields.iter().map(|item| {
                        field_pair(&item.key, &item.value, term)
                            .whitespace_nowrap()
                            .px(px(6.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::muted().opacity(0.4))
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .line_height(px(15.))
                    }),
                ))
            })
            .child(copy_line(line, logs, position, cx))
    });
    div()
        .id(("log-row", position))
        .test_support()
        .aria_label(structure.headline.clone())
        .aria_expanded(open)
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(logs) = this.logs_state() {
                logs.toggle_row(index);
                logs.list.remeasure_items(position..position + 1);
            }
            cx.notify();
        }))
        .flex()
        .flex_col()
        .px(px(12.))
        .py(px(4.))
        .hover(|row| row.bg(Tokens::muted().opacity(0.4)))
        .when(open, |row| row.bg(Tokens::muted().opacity(0.5)))
        .child(summary)
        .children(details)
}

/// The Copy line button of an open row. Its click copies the raw line and
/// does not toggle the row.
fn copy_line(
    line: &ParsedLogLine,
    logs: &LogsState,
    position: usize,
    cx: &mut Context<Inspector>,
) -> impl IntoElement {
    let index = line.index;
    let raw = line.raw.to_string();
    let copied = logs.copied_line == Some(index);
    div()
        .id(("log-copy-area", position))
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            button(Variant::Outline, 22., false)
                .id(("log-copy", position))
                .test_support()
                .aria_label("Copy line")
                .gap(px(4.))
                .px(px(6.))
                .child(ui::icon("copy", 12., Tokens::foreground()))
                .child(ui::xs(if copied { "Copied" } else { "Copy line" }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(raw.clone()));
                    if let Some(logs) = this.logs_state() {
                        logs.copied_line = Some(index);
                    }
                    cx.notify();
                })),
        )
}

/// One `LogEntry` row in the layout the preferences select.
pub(super) fn log_row(
    line: &ParsedLogLine,
    logs: &LogsState,
    position: usize,
    layout: crate::preferences::LogLayout,
    cx: &mut Context<Inspector>,
) -> ObservedElement<Stateful<Div>> {
    let structure = &line.structure;
    if layout == crate::preferences::LogLayout::OneLine {
        return one_line_row(line, logs, position, cx);
    }
    let term = logs.term.as_str();
    let wrap = logs.wrap;
    let edge = match structure.level {
        Some(LogLevel::Error) => red500(1.),
        Some(LogLevel::Warn) => tw::amber500(1.),
        _ => gpui_kit::transparent_black(),
    };
    let clip = |text: Div| if wrap { text } else { text.truncate() };
    let mut head = div().flex().min_w_0().items_start().gap(px(7.)).child(
        // `min-w-[12ch] font-mono text-[11px] leading-[17px]`.
        div()
            .min_w(px(79.2))
            .flex_none()
            .whitespace_nowrap()
            .font_family(MONO)
            .text_size(px(11.))
            .line_height(px(17.))
            .text_color(Tokens::muted_foreground())
            .child(line.time.clone()),
    );
    if logs.buffer.has_levels() {
        let (background, foreground) = structure.level.map_or(
            (gpui_kit::transparent_black(), Tokens::foreground()),
            badge_colors,
        );
        head = head.child(
            div()
                .mt(px(1.5))
                .flex()
                .flex_none()
                .justify_center()
                .min_w(px(30.))
                .px(px(4.))
                .rounded(px(3.))
                .bg(background)
                .text_color(foreground)
                .font_family(MONO)
                .text_size(px(9.5))
                .line_height(px(14.))
                .font_weight(FontWeight::BOLD)
                .child(structure.level.map_or("", LogLevel::label)),
        );
    }
    head = head.child(clip(
        div()
            .flex_1()
            .min_w_0()
            .text_size(px(12.5))
            .line_height(px(17.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(Tokens::foreground())
            .child(highlighted(&structure.headline, term)),
    ));
    if logs.is_aggregate() && !line.source_label.is_empty() {
        head = head.child(
            div()
                .ml_auto()
                .flex_none()
                .max_w(relative(0.3))
                .truncate()
                .font_family(MONO)
                .text_size(px(10.5))
                .line_height(px(17.))
                .text_color(Tokens::muted_foreground())
                .child(line.source_label.clone()),
        );
    }
    let fields = (!structure.fields.is_empty()).then(|| {
        let row = div()
            .mt(px(3.))
            .font_family(MONO)
            .text_color(Tokens::foreground().opacity(0.85));
        if wrap && layout == crate::preferences::LogLayout::Stacked {
            row.flex()
                .flex_wrap()
                .gap(px(3.))
                .children(structure.fields.iter().map(|item| {
                    field_pair(&item.key, &item.value, term)
                        .whitespace_nowrap()
                        .px(px(6.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(Tokens::border())
                        .bg(Tokens::muted().opacity(0.4))
                        .text_size(px(10.5))
                        .line_height(px(15.))
                }))
        } else {
            row.flex()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(11.))
                .line_height(px(16.))
                .children(structure.fields.iter().map(|item| {
                    field_pair(&item.key, &item.value, term)
                        .flex_none()
                        .mr(px(10.))
                }))
        }
    });
    div()
        .id(("log-row", position))
        .test_support()
        .aria_label(structure.headline.clone())
        .relative()
        .pl(px(12.))
        .pr(px(10.))
        .py(px(4.))
        .border_b_1()
        .border_color(Tokens::border().opacity(0.4))
        .hover(|row| row.bg(Tokens::muted().opacity(0.4)))
        .child(
            // `border-l-2`, drawn apart so the bottom border keeps its own color.
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(2.))
                .bg(edge),
        )
        .when(layout == crate::preferences::LogLayout::Gutter, |row| {
            row.py(px(3.))
        })
        .child(head)
        .when_some(structure.error.as_ref(), |row, error| {
            row.child(clip(
                div()
                    .mt(px(1.))
                    .when(layout == crate::preferences::LogLayout::Gutter, |row| {
                        row.ml(px(86.2 + if logs.buffer.has_levels() { 37. } else { 0. }))
                    })
                    .font_family(MONO)
                    .text_size(px(11.5))
                    .line_height(px(16.))
                    .text_color(red300())
                    .child(highlighted(error, term)),
            ))
        })
        .children(fields.map(|fields| {
            fields.when(layout == crate::preferences::LogLayout::Gutter, |row| {
                row.ml(px(86.2 + if logs.buffer.has_levels() { 37. } else { 0. }))
            })
        }))
}
