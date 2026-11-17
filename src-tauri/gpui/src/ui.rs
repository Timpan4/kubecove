//! Presentation helpers mirroring the Svelte frontend's shadcn-svelte variants
//! in `src/components/ui/svelte/classes.ts`. Sizes use the web's px values.

use crate::motion;
use crate::theme::{HEADING, Palette, SANS, SM_FONT_SIZE, Tokens, XS_FONT_SIZE};
use gpui_kit::{
    AnyElement, App, Bounds, Div, ElementId, FontWeight, GlobalElementId, Hsla, InspectorElementId,
    LayoutId, Pixels, SharedString, Styled, Svg, Transformation, Window, div, percentage,
    prelude::*, px, radians, svg,
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
/// every spinner, and rests in an inactive window and under reduced motion.
pub fn spinner(size: f32, color: Hsla) -> impl IntoElement {
    spinning(SPINNER, size, color)
}

/// An icon with `animate-spin`.
pub fn spinning(name: &str, size: f32, color: Hsla) -> impl IntoElement {
    Spinning {
        name: name.to_owned(),
        size,
        color,
    }
}

/// An icon that turns while repeating motion runs (`motion::repeats`). A
/// pending state can last for hours, so the next frame comes from the
/// repeating clock's timer, when the icon has turned one device pixel.
struct Spinning {
    name: String,
    size: f32,
    color: Hsla,
}

impl IntoElement for Spinning {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Spinning {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut icon = icon(&self.name, self.size, self.color);
        if motion::repeats(window, cx) {
            // No painted point is farther than half the icon's box from its
            // centre, so it travels at most the box's circumference per turn.
            let travel = std::f32::consts::PI * self.size * window.scale_factor();
            motion::wake_in(SPIN_TURN.div_f32(travel), window, cx);
            let turn = motion::shared_phase(SPIN_TURN, cx);
            icon = icon.with_transformation(Transformation::rotate(percentage(turn)));
        }
        let mut icon = icon.into_any_element();
        (icon.request_layout(window, cx), icon)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        icon: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        icon.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        icon: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        icon.paint(window, cx);
    }
}

/// An icon by name, where `SPINNER` names the turning spinner.
pub fn glyph(name: &str, size: f32, color: Hsla) -> AnyElement {
    if name == SPINNER {
        spinner(size, color).into_any_element()
    } else {
        icon(name, size, color).into_any_element()
    }
}

/// A branch's chevron at `open` of the way from right to down, as
/// `SidebarTreeNode.svelte` turns it: `transition-transform` to `rotate-90`.
/// Both ends use their own icon, so a still chevron is drawn as before.
pub fn chevron(size: f32, color: Hsla, open: f32) -> Svg {
    if open >= 1. {
        return icon("chevron-down", size, color);
    }
    let right = icon("chevron-right", size, color);
    if open <= 0. {
        return right;
    }
    right.with_transformation(Transformation::rotate(radians(
        open * std::f32::consts::FRAC_PI_2,
    )))
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
