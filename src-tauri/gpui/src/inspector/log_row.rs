//! One row of the Logs tab, mirroring the default stacked layout of
//! `LogEntry.svelte`: time, level badge, headline, source, error and fields.

use super::controls::red300;
use super::events::tw;
use super::logs_state::LogsState;
use crate::logs::{LogLevel, ParsedLogLine, highlight_ranges};
use crate::theme::{MONO, Tokens, oklch};
use gpui_kit::base::ObservedElement;
use gpui_kit::{
    Div, FontWeight, HighlightStyle, Hsla, SharedString, Stateful, StyledText, TestSupportExt, div,
    prelude::*, px, relative,
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

/// One stacked `LogEntry` row.
pub(super) fn log_row(
    line: &ParsedLogLine,
    logs: &LogsState,
    position: usize,
) -> ObservedElement<Stateful<Div>> {
    let structure = &line.structure;
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
        let field = |key: &SharedString, value: &SharedString| {
            div()
                .flex()
                .child(
                    div()
                        .text_color(Tokens::muted_foreground())
                        .child(highlighted(key, term)),
                )
                .child(div().w(px(6.3)))
                .child(highlighted(value, term))
        };
        let row = div()
            .mt(px(3.))
            .font_family(MONO)
            .text_color(Tokens::foreground().opacity(0.85));
        if wrap {
            row.flex()
                .flex_wrap()
                .gap(px(3.))
                .children(structure.fields.iter().map(|item| {
                    field(&item.key, &item.value)
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
                .children(
                    structure
                        .fields
                        .iter()
                        .map(|item| field(&item.key, &item.value).flex_none().mr(px(10.))),
                )
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
        .child(head)
        .when_some(structure.error.as_ref(), |row, error| {
            row.child(clip(
                div()
                    .mt(px(1.))
                    .font_family(MONO)
                    .text_size(px(11.5))
                    .line_height(px(16.))
                    .text_color(red300())
                    .child(highlighted(error, term)),
            ))
        })
        .children(fields)
}
