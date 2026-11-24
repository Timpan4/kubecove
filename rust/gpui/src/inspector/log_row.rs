//! The stacked, gutter, and one-line layouts from `LogEntry.svelte`.

use super::Inspector;
use super::controls::{Variant, button};
use super::logs_state::LogsState;
use super::overlay_motion::Tooltip;
use super::timestamp_text;
use crate::logs::{
    LogField, LogLevel, ParsedLogLine, highlight_ranges, inline_log_text, time_label,
};
use crate::model::exact_timestamp;
use crate::preferences::TimestampTimezone;
use crate::targets::ObservedElement;
use crate::targets::TestSupportExt;
use crate::theme::{MONO, Palette, Tokens, oklch};
use crate::ui;
use gpui_kit::{
    ClipboardItem, Context, Div, FontWeight, HighlightStyle, Hsla, SharedString, Stateful,
    StyledText, div, prelude::*, px, relative,
};
use std::{borrow::Cow, ops::Range};

fn red400(alpha: f32) -> Hsla {
    oklch(0.704, 0.191, 22.216, alpha)
}
fn yellow300(alpha: f32) -> Hsla {
    oklch(0.905, 0.182, 98.111, alpha)
}

/// `levelChips` active classes: (border, background, foreground).
pub(super) fn chip_colors(level: LogLevel) -> (Hsla, Hsla, Hsla) {
    match level {
        LogLevel::Error => (Palette::red500(0.45), Palette::red500(0.1), red400(1.)),
        LogLevel::Warn => (
            Palette::amber500(0.45),
            Palette::amber500(0.1),
            Palette::amber400(),
        ),
        LogLevel::Info => (
            Palette::sky500(0.45),
            Palette::sky500(0.1),
            Palette::sky300(),
        ),
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
        LogLevel::Warn => (Palette::amber500(0.15), Palette::amber400()),
        LogLevel::Info => (clear, Palette::sky300()),
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

#[derive(Default)]
struct InlineText {
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    monospace: Vec<(Range<usize>, SharedString)>,
}

impl InlineText {
    fn push(&mut self, text: &str, term: &str, style: HighlightStyle, monospace: bool) {
        let start = self.text.len();
        self.text.push_str(text);
        self.style_tail(start, term, style, monospace);
    }

    fn style_tail(&mut self, start: usize, term: &str, style: HighlightStyle, monospace: bool) {
        let end = self.text.len();
        if start == end {
            return;
        }
        if monospace {
            self.monospace
                .push((start..end, SharedString::new_static(MONO)));
        }
        let style_unmatched = style != HighlightStyle::default() || monospace;
        let mut cursor = start;
        for matched in highlight_ranges(&self.text[start..end], term) {
            let from = start + matched.start;
            if style_unmatched && cursor < from {
                self.highlights.push((cursor..from, style));
            }
            cursor = start + matched.end;
            self.highlights.push((
                from..cursor,
                HighlightStyle {
                    background_color: Some(yellow300(0.4)),
                    ..style
                },
            ));
        }
        if style_unmatched && cursor < end {
            self.highlights.push((cursor..end, style));
        }
    }

    fn fields(&mut self, fields: &[LogField], term: &str, color: Hsla) {
        let style = HighlightStyle {
            color: Some(color),
            font_weight: Some(FontWeight::NORMAL),
            ..Default::default()
        };
        for (index, field) in fields.iter().enumerate() {
            if index > 0 {
                self.text.push_str("  ");
            }
            self.field(field, term, style, false);
        }
    }

    fn field(&mut self, field: &LogField, term: &str, style: HighlightStyle, monospace: bool) {
        let start = self.text.len();
        field.write_token(&mut self.text);
        self.style_tail(start, term, style, monospace);
    }

    fn render(self) -> StyledText {
        StyledText::new(self.text)
            .with_highlights(self.highlights)
            .with_font_family_overrides(self.monospace)
    }
}

/// The row's time of day in the saved timezone. Its hover text is the exact
/// timestamp in the same timezone, as `LogEntry.svelte`'s `<time>` title is.
fn time_column(line: &ParsedLogLine, position: usize, timezone: TimestampTimezone) -> Div {
    let exact = line
        .timestamp
        .as_deref()
        .and_then(|value| exact_timestamp(value, timezone));
    div()
        .min_w(px(79.2))
        .flex_none()
        .whitespace_nowrap()
        .font_family(MONO)
        .text_size(px(11.))
        .line_height(px(17.))
        .text_color(Tokens::muted_foreground())
        .child(timestamp_text::with_tooltip(
            format!("log-time:{position}"),
            exact,
            div().child(time_label(line.timestamp.as_deref(), timezone)),
        ))
}

fn badge(level: Option<LogLevel>) -> Div {
    let (background, foreground) = level.map_or(
        (gpui_kit::transparent_black(), Tokens::foreground()),
        badge_colors,
    );
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
        .child(level.map_or("", LogLevel::label))
}

fn row_edge(level: Option<LogLevel>) -> Div {
    let color = match level {
        Some(LogLevel::Error) => Palette::red500(1.),
        Some(LogLevel::Warn) => Palette::amber500(1.),
        _ => gpui_kit::transparent_black(),
    };
    div()
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .w(px(2.))
        .bg(color)
}

fn field_chips(fields: &[LogField], term: &str) -> Div {
    div()
        .flex()
        .flex_wrap()
        .w_full()
        .min_w_0()
        .gap(px(3.))
        .font_family(MONO)
        .text_color(Tokens::foreground().opacity(0.85))
        .children(fields.iter().enumerate().map(|(index, item)| {
            let mut text = InlineText::default();
            text.field(item, term, HighlightStyle::default(), false);
            div()
                .id(("log-field", index))
                .test_support()
                .max_w_full()
                .min_w_0()
                .px(px(6.))
                .rounded(px(4.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::muted().opacity(0.4))
                .text_size(px(10.5))
                .line_height(px(15.))
                .child(text.render())
        }))
}

fn compact_summary(
    line: &ParsedLogLine,
    headline: &SharedString,
    term: &str,
    open: bool,
) -> StyledText {
    let structure = &line.structure;
    if open || (structure.error.is_none() && structure.fields.is_empty()) {
        return highlighted(headline, term);
    }
    let mut text = InlineText::default();
    text.push(headline, term, HighlightStyle::default(), false);
    if let Some(error) = &structure.error {
        text.text.push_str("  ");
        text.push(
            &inline_log_text(error),
            term,
            HighlightStyle {
                color: Some(Palette::red300()),
                font_weight: Some(FontWeight::NORMAL),
                ..Default::default()
            },
            true,
        );
    }
    let field_style = HighlightStyle {
        color: Some(Tokens::foreground().opacity(0.75)),
        font_weight: Some(FontWeight::NORMAL),
        ..Default::default()
    };
    for field in &structure.fields {
        text.text.push_str("  ");
        text.field(field, term, field_style, true);
    }
    text.render()
}

/// `LogEntry.svelte`'s `oneLine` row. The summary line toggles the row. An
/// open row drops the inline error and fields for a block with the error,
/// the field chips and Copy line.
fn one_line_row(
    line: &ParsedLogLine,
    logs: &LogsState,
    position: usize,
    timezone: TimestampTimezone,
    cx: &mut Context<Inspector>,
) -> ObservedElement<Stateful<Div>> {
    let structure = &line.structure;
    let index = line.index;
    let open = logs.row_open(index);
    let headline = if open {
        structure.headline.clone()
    } else {
        match inline_log_text(&structure.headline) {
            Cow::Borrowed(_) => structure.headline.clone(),
            Cow::Owned(text) => text.into(),
        }
    };
    // The handle is the tab stop: an element's `tab_index` applies only to a
    // handle the element creates, not to one it tracks.
    let focus = logs
        .row_focus
        .borrow_mut()
        .entry(index)
        .or_insert_with(|| cx.focus_handle().tab_stop(true))
        .clone();
    let term = logs.term.as_str();
    let source = (logs.is_aggregate() && !line.source_label.is_empty()).then(|| {
        let label = line.source_label.clone();
        div()
            .id(("log-source", position))
            .test_support()
            .aria_label(label.clone())
            .flex_none()
            .max_w(relative(0.3))
            .truncate()
            .font_family(MONO)
            .text_size(px(10.5))
            .line_height(px(18.))
            .text_color(Tokens::muted_foreground())
            .child(label.clone())
            .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
    });
    let summary = div()
        .id(("log-summary", position))
        .test_support()
        .min_w_0()
        .text_size(px(12.))
        .line_height(px(18.))
        .font_weight(FontWeight::MEDIUM)
        .when(!open, |summary| summary.flex_1().truncate())
        .when(open, |summary| summary.w_full())
        .child(compact_summary(line, &headline, term, open));
    let details = open.then(|| {
        div()
            .id(("log-details", position))
            .test_support()
            .flex()
            .flex_col()
            .items_start()
            .w_full()
            .min_w_0()
            .gap(px(4.))
            .pt(px(2.))
            .pb(px(4.))
            .children(structure.error.as_ref().map(|error| {
                div()
                    .font_family(MONO)
                    .text_size(px(11.5))
                    .line_height(px(16.))
                    .text_color(Palette::red300())
                    .child(highlighted(error, term))
            }))
            .when(!structure.fields.is_empty(), |details| {
                details.child(field_chips(&structure.fields, term))
            })
            .child(copy_line(line, logs, position, cx))
    });
    div()
        .id(("log-row", position))
        .test_support()
        .aria_label(headline)
        .aria_expanded(open)
        .track_focus(&focus)
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(logs) = this.logs_state() {
                logs.toggle_row(index);
                logs.list.remeasure_items(position..position + 1);
            }
            cx.notify();
        }))
        .relative()
        .flex()
        .w_full()
        .items_start()
        .gap(px(7.))
        .pl(px(12.))
        .pr(px(10.))
        .py(px(2.))
        .cursor_pointer()
        .hover(|row| row.bg(Tokens::muted().opacity(0.4)))
        .focus_visible(|row| row.bg(Tokens::muted().opacity(0.4)))
        .when(open, |row| row.bg(Tokens::muted().opacity(0.5)))
        .child(row_edge(structure.level))
        .child(time_column(line, position, timezone))
        .when(logs.buffer.has_levels(), |row| {
            row.child(badge(structure.level))
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .flex()
                        // An expanded aggregate row gives the message the full width.
                        .when(open, |row| row.flex_col())
                        .when(!open, |row| row.gap(px(8.)))
                        .items_start()
                        .children(source)
                        .child(summary),
                )
                .children(details),
        )
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
    timezone: TimestampTimezone,
    cx: &mut Context<Inspector>,
) -> ObservedElement<Stateful<Div>> {
    let structure = &line.structure;
    if layout == crate::preferences::LogLayout::OneLine {
        return one_line_row(line, logs, position, timezone, cx);
    }
    let term = logs.term.as_str();
    let gutter = layout == crate::preferences::LogLayout::Gutter;
    let clip = |text: Div| if logs.wrap { text } else { text.truncate() };
    let headline = clip(
        div()
            .flex_1()
            .min_w_0()
            .text_size(px(12.5))
            .line_height(px(17.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(Tokens::foreground())
            .child(highlighted(&structure.headline, term)),
    );
    let source = (logs.is_aggregate() && !line.source_label.is_empty()).then(|| {
        div()
            .ml_auto()
            .flex_none()
            .max_w(relative(0.3))
            .truncate()
            .font_family(MONO)
            .text_size(px(10.5))
            .line_height(px(17.))
            .text_color(Tokens::muted_foreground())
            .child(line.source_label.clone())
    });
    let error = structure.error.as_ref().map(|error| {
        clip(
            div()
                .when(!gutter, |row| row.mt(px(1.)))
                .font_family(MONO)
                .text_size(px(11.5))
                .line_height(px(16.))
                .text_color(Palette::red300())
                .child(highlighted(error, term)),
        )
    });
    let fields = (!structure.fields.is_empty()).then(|| {
        let row = if logs.wrap && !gutter {
            field_chips(&structure.fields, term)
        } else {
            let mut text = InlineText::default();
            text.fields(&structure.fields, term, Tokens::foreground().opacity(0.85));
            clip(
                div()
                    .font_family(MONO)
                    .text_size(px(11.))
                    .line_height(px(16.))
                    .child(text.render()),
            )
        };
        row.when(!gutter, |row| row.mt(px(3.)))
    });
    let columns = div()
        .flex()
        .min_w_0()
        .items_start()
        .gap(px(if gutter { 8. } else { 7. }))
        .child(time_column(line, position, timezone))
        .when(logs.buffer.has_levels(), |row| {
            row.child(badge(structure.level))
        });
    let row = div()
        .id(("log-row", position))
        .test_support()
        .aria_label(structure.headline.clone())
        .relative()
        .pl(px(12.))
        .pr(px(10.))
        .py(px(4.))
        .border_b_1()
        .border_color(Tokens::border().opacity(if gutter { 0.3 } else { 0.4 }))
        .hover(|row| row.bg(Tokens::muted().opacity(0.4)))
        .child(row_edge(structure.level));
    if gutter {
        row.py(px(3.)).child(
            columns.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(8.))
                            .child(headline)
                            .children(source),
                    )
                    .children(error)
                    .children(fields),
            ),
        )
    } else {
        row.child(columns.child(headline).children(source))
            .children(error)
            .children(fields)
    }
}
