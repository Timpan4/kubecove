//! Buttons and diff lines the detail tabs share, from `buttonVariants` in
//! `src/components/ui/svelte/classes.ts` and `diffLineClassName` in
//! `yamlTabDiff.ts`.

use super::windowed::Windowed;
use crate::control_motion::{self, HoverFill, hover_fill};
use crate::theme::{MONO, Palette, Tokens};
use crate::ui;
use crate::unified_diff::{DiffLine, DiffLineKind};
use gpui_kit::{Div, FontWeight, HighlightStyle, Hsla, SharedString, div, prelude::*, px};
use std::ops::Range;
use std::sync::Arc;

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
/// `text-foreground` over `text-muted-foreground`, mixed as its
/// `transition-[color]` does. The option's label inherits the text color, and
/// the group's [`segment_pill`] carries the active background.
pub(super) fn segment<E: Styled>(option: E, on: f32) -> E {
    option.text_color(control_motion::mix(
        Tokens::muted_foreground(),
        Tokens::foreground(),
        on,
    ))
}

/// The `bg-surface-1 shadow-sm` pill of `SegmentedControl.svelte`, which slides
/// to the active option: the first child of a `relative` group whose options
/// go through `slide.option`. `id` names the group and `active` is the active
/// option's place in it.
pub(super) fn segment_pill(
    slide: &control_motion::Slide,
    id: impl Into<gpui_kit::ElementId>,
    active: usize,
) -> control_motion::SlideIndicator {
    slide
        .indicator(id, active, Tokens::card())
        .rounded(px(RADIUS))
        .shadow_sm()
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
        DiffLineKind::Add => (Palette::emerald500(0.12), Palette::emerald300()),
        DiffLineKind::Remove => (Tokens::destructive().opacity(0.12), Palette::red300()),
        DiffLineKind::Hunk => (Tokens::primary().opacity(0.1), Tokens::primary()),
        DiffLineKind::Header => (Tokens::muted().opacity(0.35), Tokens::muted_foreground()),
        DiffLineKind::Context | DiffLineKind::Meta | DiffLineKind::Empty => {
            (clear, Tokens::muted_foreground())
        }
    }
}

/// One block diff line: `<div class="whitespace-pre px-3 {diffLineClassName}">`,
/// so its tint spans the full width. `highlights` color its YAML syntax.
pub(super) fn diff_row(
    line: &DiffLine,
    height: f32,
    highlights: Option<&[(Range<usize>, HighlightStyle)]>,
) -> Div {
    let (background, foreground) = diff_colors(line.kind);
    let row = div()
        .h(px(height))
        .px(px(12.))
        .whitespace_nowrap()
        .bg(background)
        .text_color(foreground)
        .when(line.kind == DiffLineKind::Header, |row| {
            row.font_weight(FontWeight::SEMIBOLD)
        });
    match highlights {
        Some(highlights) => row.child(
            gpui_kit::StyledText::new(line.text.clone())
                .with_highlights(highlights.iter().cloned()),
        ),
        None => row.child(line.text.clone()),
    }
}

pub(super) type DiffHighlights = Arc<[Vec<(Range<usize>, HighlightStyle)>]>;

/// Block diff lines `height` high, built only where the ancestor scroller's
/// clip shows them, so a full diff of any length costs a viewport of rows.
/// `highlights` holds each line's syntax colors, by line index.
pub(super) fn windowed_diff_rows(
    lines: Arc<[DiffLine]>,
    height: f32,
    highlights: Option<DiffHighlights>,
) -> Div {
    let tops = (0..=lines.len()).map(|row| row as f32 * height).collect();
    Windowed::fixed_rows(tops, move |row, _, _| {
        let colors = highlights
            .as_ref()
            .and_then(|highlights| highlights.get(row))
            .map(Vec::as_slice);
        diff_row(&lines[row], height, colors).into_any_element()
    })
}

/// The YAML dry-run diff in its scroller.
pub(super) fn diff_lines(lines: Arc<[DiffLine]>, line_height: f32) -> impl IntoElement {
    div()
        .font_family(MONO)
        .text_size(px(12.))
        .line_height(px(line_height))
        .child(windowed_diff_rows(lines, line_height, None))
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
