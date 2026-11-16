//! Presentation helpers mirroring the Svelte frontend's shadcn-svelte variants
//! in `src/components/ui/svelte/classes.ts`. Sizes use the web's px values.

use crate::theme::{HEADING, Palette, SANS, SM_FONT_SIZE, Tokens, XS_FONT_SIZE};
use gpui_kit::{
    Animation, AnimationExt, AnyElement, Div, FontWeight, Hsla, Pixels, SharedString, Styled, Svg,
    Transformation, div, percentage, prelude::*, px, svg,
};
use std::time::Duration;

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
    text(content, XS_FONT_SIZE, 16.)
}

/// `text-sm` (14px / 20px).
pub fn sm(content: impl Into<SharedString>) -> Div {
    text(content, SM_FONT_SIZE, 20.)
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

/// The icon `Spinner.svelte` draws.
pub const SPINNER: &str = "loader-circle";

/// Tailwind's `animate-spin`, which `Spinner.svelte` sets: `spin 1s linear infinite`
/// in `node_modules/tailwindcss/theme.css`.
const SPIN_TURN: Duration = Duration::from_secs(1);

/// `Spinner.svelte`. It turns only while it is rendered, on one clock shared by
/// every spinner, and stays still under reduced motion.
pub fn spinner(size: f32, color: Hsla) -> impl IntoElement {
    spinning(SPINNER, size, color)
}

/// An icon with `animate-spin`.
pub fn spinning(name: &str, size: f32, color: Hsla) -> impl IntoElement {
    icon(name, size, color).with_animation(
        "spin",
        Animation::new(SPIN_TURN).repeat_synced(),
        |icon, turn| icon.with_transformation(Transformation::rotate(percentage(turn))),
    )
}

/// An icon by name, where `SPINNER` names the turning spinner.
pub fn glyph(name: &str, size: f32, color: Hsla) -> AnyElement {
    if name == SPINNER {
        spinner(size, color).into_any_element()
    } else {
        icon(name, size, color).into_any_element()
    }
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
fn badge_frame(border: Hsla, background: Hsla) -> Div {
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
}

fn badge_text(content: impl Into<SharedString>, foreground: Hsla) -> Div {
    text(content, 10., 14.)
        .whitespace_nowrap()
        .font_weight(FontWeight::MEDIUM)
        .text_color(foreground)
}

fn badge(
    border: Hsla,
    background: Hsla,
    foreground: Hsla,
    content: impl Into<SharedString>,
) -> Div {
    badge_frame(border, background).child(badge_text(content, foreground).flex_none())
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

/// `Badge variant="outline"` with `max-w-* truncate`: a label wider than
/// `max_width` ends in an ellipsis.
pub fn truncated_outline_badge(content: impl Into<SharedString>, max_width: Pixels) -> Div {
    badge_frame(Tokens::border(), Tokens::input().opacity(0.3))
        .max_w(max_width)
        .justify_start()
        .child(
            badge_text(content, Tokens::foreground())
                .min_w_0()
                .truncate(),
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
    ghost_icon_spinning(name, size, icon_size, false)
}

/// `ghost_icon` whose icon has `animate-spin` while `spin` is set.
pub fn ghost_icon_spinning(name: &str, size: f32, icon_size: f32, spin: bool) -> Div {
    let frame = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(8.));
    if spin {
        frame.child(spinning(name, icon_size, Tokens::foreground()))
    } else {
        frame.child(icon(name, icon_size, Tokens::foreground()))
    }
}
