//! Presentation helpers mirroring the Svelte frontend's shadcn-svelte variants
//! in `src/components/ui/svelte/classes.ts`. Sizes use the web's px values.

use crate::theme::{HEADING, Palette, SANS, Tokens};
use gpui_kit::{Div, FontWeight, Hsla, SharedString, Styled, Svg, div, prelude::*, px, svg};

/// Text at a web font size (px) and line height (px).
pub fn text(content: impl Into<SharedString>, size: f32, line: f32) -> Div {
    div()
        .font_family(SANS)
        .text_size(px(size))
        .line_height(px(line))
        .child(content.into())
}

/// `text-xs` (12px / 16px).
pub fn xs(content: impl Into<SharedString>) -> Div {
    text(content, 12., 16.)
}

/// `text-sm` (14px / 20px).
pub fn sm(content: impl Into<SharedString>) -> Div {
    text(content, 14., 20.)
}

pub fn muted(element: Div) -> Div {
    element.text_color(Tokens::muted_foreground())
}

pub fn semibold(element: Div) -> Div {
    element.font_weight(FontWeight::SEMIBOLD)
}

pub fn medium(element: Div) -> Div {
    element.font_weight(FontWeight::MEDIUM)
}

/// `font-heading text-sm font-medium` card titles.
pub fn heading(content: impl Into<SharedString>, size: f32, line: f32) -> Div {
    text(content, size, line)
        .font_family(HEADING)
        .font_weight(FontWeight::MEDIUM)
}

/// Uppercase section label, `text-[10.5px] font-semibold uppercase tracking-wide`.
pub fn caps(content: &str, size: f32) -> Div {
    text(content.to_uppercase(), size, size * 1.4)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(Tokens::muted_foreground())
}

pub fn icon(name: &str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(SharedString::from(format!("app-icons/{name}.svg")))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// `Card`: `rounded-lg border bg-surface-1`.
pub fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .overflow_hidden()
}

/// Inner panel, `rounded-md border bg-background/30`.
pub fn panel() -> Div {
    div()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.3))
}

/// Badge base: `inline-flex h-5 items-center justify-center overflow-hidden
/// whitespace-nowrap px-2 text-[10px] font-medium`. Overflowing text stays
/// centered and clips on both sides, as in the web.
fn badge(
    border: Hsla,
    background: Hsla,
    foreground: Hsla,
    content: impl Into<SharedString>,
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
            text(content, 10., 14.)
                .flex_none()
                .whitespace_nowrap()
                .font_weight(FontWeight::MEDIUM)
                .text_color(foreground),
        )
}

/// `Badge variant="outline"`.
pub fn outline_badge(content: impl Into<SharedString>) -> Div {
    badge(
        Tokens::border(),
        Tokens::input().opacity(0.3),
        Tokens::foreground(),
        content,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Success,
    Warning,
    Error,
}

/// Status badges from `src/components/status-badge-styles.ts`.
pub fn status_badge(tone: Tone, content: impl Into<SharedString>) -> Div {
    let (border, background, foreground) = match tone {
        Tone::Success => (
            Palette::emerald500(0.3),
            Palette::emerald500(0.15),
            Palette::emerald300(),
        ),
        Tone::Warning => (
            Palette::amber500(0.3),
            Palette::amber500(0.15),
            Palette::amber300(),
        ),
        Tone::Error => (
            gpui_kit::transparent_black(),
            Tokens::destructive().opacity(0.2),
            Tokens::destructive(),
        ),
        Tone::Neutral => (
            gpui_kit::transparent_black(),
            Tokens::secondary(),
            Tokens::foreground(),
        ),
    };
    badge(border, background, foreground, content)
}

/// `Button variant="outline"` body; the caller adds id, size and handlers.
pub fn outline_button() -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::input().opacity(0.3))
        .text_color(Tokens::foreground())
}

/// `Button variant="ghost" size="icon"` (28px square).
pub fn ghost_icon(name: &str, size: f32, icon_size: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(8.))
        .child(icon(name, icon_size, Tokens::foreground()))
}
