//! Buttons and diff lines the detail tabs share, from `buttonVariants` in
//! `src/components/ui/svelte/classes.ts` and `diffLineClassName` in
//! `yamlTabDiff.ts`.

use super::events::tw;
use crate::control_motion::{self, HoverFill, hover_fill};
use crate::theme::{MONO, Tokens, oklch};
use crate::ui;
use crate::unified_diff::{DiffLine, DiffLineKind};
use gpui_kit::{Div, FontWeight, Hsla, SharedString, div, prelude::*, px};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Variant {
    Default,
    Outline,
    Secondary,
    Ghost,
    Destructive,
}

/// `rounded-md`, the radius of every button size but `xs`.
pub(super) const RADIUS: f32 = 8.;

struct Colors {
    border: Hsla,
    background: Hsla,
    /// The variant's `hover:bg-*` class, in its `dark:` form where it has one.
    hover: Hsla,
    foreground: Hsla,
}

impl Variant {
    fn colors(self) -> Colors {
        let clear = gpui_kit::transparent_black();
        match self {
            Variant::Default => Colors {
                border: clear,
                background: Tokens::primary(),
                hover: Tokens::primary().opacity(0.8),
                foreground: Tokens::primary_foreground(),
            },
            Variant::Outline => Colors {
                border: Tokens::border(),
                background: Tokens::input().opacity(0.3),
                hover: Tokens::input().opacity(0.5),
                foreground: Tokens::foreground(),
            },
            Variant::Secondary => Colors {
                border: clear,
                background: Tokens::secondary(),
                hover: Tokens::secondary().opacity(0.8),
                foreground: Tokens::foreground(),
            },
            Variant::Ghost => Colors {
                border: clear,
                background: clear,
                hover: Tokens::muted().opacity(0.5),
                foreground: Tokens::foreground(),
            },
            Variant::Destructive => Colors {
                border: clear,
                background: Tokens::destructive().opacity(0.2),
                hover: Tokens::destructive().opacity(0.3),
                foreground: Tokens::destructive(),
            },
        }
    }

    /// The variant's background for a button of `radius`, which fades to its
    /// hover color as `buttonVariants`' `transition-[color,background-color,…]`
    /// does. The default and secondary variants also gain `hover:shadow-sm`.
    pub(super) fn fill(self, radius: f32) -> HoverFill {
        self.fill_over(self.colors().background, radius)
    }

    /// `fill` from another resting background, such as a pressed state's. The
    /// variant's `hover:bg-*` class still wins while hovered, as on the web.
    pub(super) fn fill_over(self, rest: Hsla, radius: f32) -> HoverFill {
        let fill = hover_fill(rest, self.colors().hover)
            .rounded(px(radius))
            .under_border(1., 1., 1., 1.);
        if matches!(self, Variant::Default | Variant::Secondary) {
            fill.shadow_sm()
        } else {
            fill
        }
    }
}

/// `h-7` (default) or `h-6` (`sm`): `gap-1 px-2 text-xs/relaxed font-medium`.
pub(super) fn button(variant: Variant, height: f32, disabled: bool) -> Div {
    let frame = button_frame(variant, height, disabled);
    if disabled {
        // `disabled:pointer-events-none`: no hover.
        frame.bg(variant.colors().background)
    } else {
        frame.child(variant.fill(RADIUS))
    }
}

/// A button that does not answer the pointer and is not dimmed, for a wrapper
/// that marks it unavailable itself.
pub(super) fn inert_button(variant: Variant, height: f32) -> Div {
    button_frame(variant, height, false).bg(variant.colors().background)
}

/// A button's box without its background, for a caller that paints its own:
/// a pressed state, another radius, or `Variant::fill` at that radius.
pub(super) fn button_frame(variant: Variant, height: f32, disabled: bool) -> Div {
    let Colors {
        border, foreground, ..
    } = variant.colors();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .h(px(height))
        .px(px(8.))
        .rounded(px(RADIUS))
        .border_1()
        .border_color(border)
        .text_color(foreground)
        .font_family(crate::theme::SANS)
        .text_size(px(12.))
        .line_height(px(19.5))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .when(disabled, |button| button.opacity(0.5))
        .when(!disabled, |button| button.cursor_pointer())
}

/// One `SegmentedControl.svelte` option at `on` of the way to active:
/// `bg-surface-1 text-foreground shadow-sm` over `text-muted-foreground`, mixed
/// as its `transition-[color,background-color,border-color,box-shadow]` does.
/// The option's label inherits the text color.
pub(super) fn segment<E: Styled>(option: E, on: f32) -> E {
    option
        .bg(control_motion::mix(
            gpui_kit::transparent_black(),
            Tokens::card(),
            on,
        ))
        .text_color(control_motion::mix(
            Tokens::muted_foreground(),
            Tokens::foreground(),
            on,
        ))
        .shadow(control_motion::shadow_sm(on))
}

/// `Button size="icon"` (28px) around a 14px icon.
pub(super) fn icon_button(name: &str, color: Hsla) -> Div {
    button(Variant::Outline, 28., false)
        .w(px(28.))
        .px_0()
        .child(ui::icon(name, 14., color))
}

/// `diffLineClassName`: (background, foreground).
pub(super) fn diff_colors(kind: DiffLineKind) -> (Hsla, Hsla) {
    let clear = gpui_kit::transparent_black();
    match kind {
        DiffLineKind::Add => (tw::emerald500(0.12), tw::emerald300()),
        DiffLineKind::Remove => (Tokens::destructive().opacity(0.12), red300()),
        DiffLineKind::Hunk => (Tokens::primary().opacity(0.1), Tokens::primary()),
        DiffLineKind::Header => (Tokens::muted().opacity(0.35), Tokens::muted_foreground()),
        DiffLineKind::Context | DiffLineKind::Meta | DiffLineKind::Empty => {
            (clear, Tokens::muted_foreground())
        }
    }
}

pub(super) fn red300() -> Hsla {
    oklch(0.808, 0.114, 19.571, 1.)
}

/// The YAML dry-run diff: `<div class="whitespace-pre px-3 {diffLineClassName}">`
/// block lines, so each tint spans the full width.
pub(super) fn diff_lines(lines: &[DiffLine], line_height: f32) -> impl IntoElement {
    div()
        .font_family(MONO)
        .text_size(px(12.))
        .line_height(px(line_height))
        .children(lines.iter().map(|line| {
            let (background, foreground) = diff_colors(line.kind);
            div()
                .h(px(line_height))
                .px(px(12.))
                .whitespace_nowrap()
                .bg(background)
                .text_color(foreground)
                .when(line.kind == DiffLineKind::Header, |text| {
                    text.font_weight(FontWeight::SEMIBOLD)
                })
                .child(line.text.clone())
        }))
}

/// `<pre class="text-xs leading-5">` of `<span class={diffLineClassName}>`:
/// inline spans, so each tint covers only its text.
pub(super) fn diff_pre(id: impl Into<SharedString>, lines: &[DiffLine]) -> impl IntoElement {
    let id: SharedString = id.into();
    div()
        .id(gpui_kit::ElementId::Name(id))
        .overflow_x_scroll()
        .p(px(12.))
        .font_family(MONO)
        .text_size(px(12.))
        .line_height(px(20.))
        .children(lines.iter().map(|line| {
            let (background, foreground) = diff_colors(line.kind);
            div().flex().h(px(20.)).child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .bg(background)
                    .text_color(foreground)
                    .when(line.kind == DiffLineKind::Header, |text| {
                        text.font_weight(FontWeight::SEMIBOLD)
                    })
                    .child(line.text.clone()),
            )
        }))
}
