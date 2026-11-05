//! Buttons and diff lines the detail tabs share, from `buttonVariants` in
//! `src/components/ui/svelte/classes.ts` and `diffLineClassName` in
//! `yamlTabDiff.ts`.

use super::events::tw;
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

/// `h-7` (default) or `h-6` (`sm`): `gap-1 px-2 text-xs/relaxed font-medium`.
pub(super) fn button(variant: Variant, height: f32, disabled: bool) -> Div {
    let (border, background, foreground) = match variant {
        Variant::Default => (
            gpui_kit::transparent_black(),
            Tokens::primary(),
            Tokens::primary_foreground(),
        ),
        Variant::Outline => (
            Tokens::border(),
            Tokens::input().opacity(0.3),
            Tokens::foreground(),
        ),
        Variant::Secondary => (
            gpui_kit::transparent_black(),
            Tokens::secondary(),
            Tokens::foreground(),
        ),
        Variant::Ghost => (
            gpui_kit::transparent_black(),
            gpui_kit::transparent_black(),
            Tokens::foreground(),
        ),
        Variant::Destructive => (
            gpui_kit::transparent_black(),
            Tokens::destructive().opacity(0.2),
            Tokens::destructive(),
        ),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .h(px(height))
        .px(px(8.))
        .rounded(px(8.))
        .border_1()
        .border_color(border)
        .bg(background)
        .text_color(foreground)
        .font_family(crate::theme::SANS)
        .text_size(px(12.))
        .line_height(px(19.5))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .when(disabled, |button| button.opacity(0.5))
        .when(!disabled, |button| button.cursor_pointer())
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
