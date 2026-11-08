//! Pieces the GitOps and Helm pages share: the rail, `SegmentedControl.svelte`, `Empty`, and the card grid that
//! `VirtualCollection.svelte` lays out. GPUI has no container queries, so the
//! `@container` breakpoints use the frame width derived from the window.

use super::{Inspector, controls, resources};
use crate::targets::TestSupportExt;
use crate::theme::{HEADING, Palette, Tokens};
use crate::ui;
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, Hsla, ScrollHandle, SharedString, Window, div,
    prelude::*, px,
};

/// The `gitOpsViewMode` and `helmViewMode` settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ViewMode {
    #[default]
    Cards,
    List,
}

/// `@min-[64rem]`: the rail sits beside the grid.
const WIDE: f32 = 1024.;
/// `14rem` rail track plus the `gap-4` beside it.
pub(super) const RAIL: f32 = 224.;
const RAIL_GAP: f32 = 16.;
/// `gap-3` between cards and card rows.
pub(super) const CARD_GAP: f32 = 12.;
/// `h-[18.5rem]`.
pub(super) const CARD_HEIGHT: f32 = 296.;

/// Virtual rows share one horizontal offset so columns stay aligned as rows mount.
pub(super) fn horizontal(
    body: Div,
    width: f32,
    minimum: f32,
    scroll: &ScrollHandle,
    id: String,
) -> Div {
    div().min_w_0().child(
        div()
            .id(SharedString::from(id))
            .w_full()
            .overflow_x_scroll()
            .track_scroll(scroll)
            .child(body.w(px(width.max(minimum)))),
    )
}

/// The page's container-query layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Layout {
    /// `@min-[42rem]` puts the five summary facts on one row.
    pub summary_columns: usize,
    pub rail_beside: bool,
    /// The card grid's width.
    pub grid: f32,
    pub columns: usize,
    pub card: f32,
}

impl Layout {
    /// The `VirtualCollection` card grid: one column, `@min-[35rem]` two, and
    /// `@min-[64rem]` `repeat(auto-fit,minmax(18.75rem,26.25rem))`, whose probe
    /// holds `max(2, ceil(width / 300))` cells, capped at the item count.
    pub(super) fn new(container: f32, items: usize) -> Self {
        let rail_beside = container >= WIDE;
        let grid = if rail_beside {
            container - RAIL - RAIL_GAP
        } else {
            container
        };
        let columns = if container >= WIDE {
            let fit = ((grid + CARD_GAP) / (300. + CARD_GAP)).floor().max(1.) as usize;
            let probe = items.min(((grid / 300.).ceil() as usize).max(2));
            fit.min(probe).max(1)
        } else if container >= 560. {
            2
        } else {
            1
        };
        let share = (grid - CARD_GAP * (columns as f32 - 1.)) / columns as f32;
        Self {
            summary_columns: if container >= 672. { 5 } else { 2 },
            rail_beside,
            grid,
            columns,
            card: if container >= WIDE {
                share.min(420.)
            } else {
                share
            },
        }
    }
}

impl Inspector {
    /// The `SurfaceFrame` content width: the page minus `px-5`.
    pub(super) fn frame_width(&self, window: &Window) -> f32 {
        resources::page_width(self, window) - 40.
    }
}

/// `SurfaceFrame.svelte`'s header with its `headerActions`.
pub(super) fn frame_header(icon: &str, title: &str, actions: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(ui::icon(icon, 20., Tokens::muted_foreground()))
        .child(
            ui::text(title.to_owned(), 16., 24.)
                .font_family(HEADING)
                .font_weight(FontWeight::SEMIBOLD),
        )
        .children(actions)
}

/// `text-xs font-semibold uppercase text-muted-foreground` rail heading.
pub(super) fn rail_heading(label: &str) -> Div {
    ui::semibold(ui::muted(ui::xs(label.to_uppercase())))
        .px(px(4.))
        .whitespace_nowrap()
}

/// A rail row: `rounded-md px-2.5 py-2 text-sm` with a count chip; the active
/// row is `bg-accent font-medium`, a disabled row `opacity-60`.
pub(super) fn rail_item(label: &str, count: usize, active: bool, disabled: bool) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .px(px(10.))
        .py(px(8.))
        .rounded(px(8.))
        // `bg-accent`, which the dark theme sets to the muted color.
        .when(active, |row| row.bg(Tokens::muted()))
        .when(disabled && !active, |row| row.opacity(0.6))
        .child(
            ui::sm(label.to_owned())
                .min_w_0()
                .truncate()
                .text_color(if active {
                    Tokens::foreground()
                } else {
                    Tokens::muted_foreground()
                })
                .when(active, |text| text.font_weight(FontWeight::MEDIUM)),
        )
        .child(
            ui::xs(count.to_string())
                .line_height(px(12.))
                .px(px(6.))
                .py(px(2.))
                .rounded(px(4.))
                .bg(Tokens::muted())
                .text_color(if active {
                    Tokens::foreground()
                } else {
                    Tokens::muted_foreground()
                }),
        )
}

/// The rail column: `border-r pr-4` beside the grid, or `border-b pb-3`
/// above it as a horizontal strip.
pub(super) fn rail(beside: bool, sections: Vec<Div>) -> Div {
    if beside {
        div()
            .flex_none()
            .w(px(RAIL))
            .pr(px(16.))
            .border_r_1()
            .border_color(Tokens::border())
            .flex()
            .flex_col()
            .gap(px(20.))
            .children(sections)
    } else {
        div()
            .flex()
            .gap(px(20.))
            .pb(px(12.))
            .border_b_1()
            .border_color(Tokens::border())
            .overflow_hidden()
            .children(sections)
    }
}

/// A rail section: the heading over its rows, or beside them in a strip.
pub(super) fn rail_section(beside: bool, heading: &str, rows: Vec<AnyElement>) -> Div {
    if beside {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(rail_heading(heading))
            .child(div().flex().flex_col().gap(px(4.)).children(rows))
    } else {
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .child(rail_heading(heading))
            .child(div().flex().gap(px(4.)).children(rows))
    }
}

/// The empty rail cell beside later card rows, which carries the `border-r`.
pub(super) fn rail_spacer() -> Div {
    div()
        .flex_none()
        .w(px(RAIL))
        .border_r_1()
        .border_color(Tokens::border())
}

/// `SegmentedControl` with `Cards` and `List`.
pub(super) fn view_toggle(
    id: &str,
    mode: ViewMode,
    set: fn(&mut Inspector, ViewMode),
    cx: &mut Context<Inspector>,
) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .p(px(2.))
        .rounded(px(10.))
        .bg(Tokens::background())
        .border_1()
        .border_color(Tokens::border().opacity(0.6))
        .children(
            [(ViewMode::Cards, "Cards"), (ViewMode::List, "List")].map(|(option, label)| {
                let active = option == mode;
                div()
                    .id(SharedString::from(format!("{id}:{label}")))
                    .test_support()
                    .aria_label(label)
                    .aria_selected(active)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .h(px(24.))
                    .px(px(8.))
                    .rounded(px(8.))
                    .when(active, |button| button.bg(Tokens::card()).shadow_sm())
                    .child(ui::medium(ui::xs(label)).text_color(if active {
                        Tokens::foreground()
                    } else {
                        Tokens::muted_foreground()
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        set(this, option);
                        this.persisted.preferences.git_ops_view_mode =
                            if this.gitops_page.mode == ViewMode::Cards {
                                crate::preferences::DeliveryMode::Cards
                            } else {
                                crate::preferences::DeliveryMode::List
                            };
                        this.persisted.preferences.helm_view_mode =
                            if this.helm_page.mode == ViewMode::Cards {
                                crate::preferences::DeliveryMode::Cards
                            } else {
                                crate::preferences::DeliveryMode::List
                            };
                        this.save_preferences(cx);
                        cx.notify();
                    }))
            }),
        )
}

/// `Empty` with `EmptyTitle` and `EmptyDescription`, no border, and an
/// optional `EmptyContent` below them.
pub(super) fn empty(
    id: &str,
    title: &str,
    description: &str,
    content: Option<AnyElement>,
) -> impl IntoElement + use<> {
    div()
        .id(SharedString::from(id.to_owned()))
        .test_support()
        .aria_label(title.to_owned())
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .p(px(24.))
        .w_full()
        .child(
            ui::text(title.to_owned(), 14., 20.)
                .font_family(HEADING)
                .font_weight(FontWeight::MEDIUM),
        )
        .child(ui::muted(ui::text(description.to_owned(), 12., 19.5)))
        .children(content.map(|content| div().mt(px(12.)).child(content)))
}

/// The `Badge` box (`h-5 rounded-full border px-2 text-[10px] font-medium`)
/// in explicit colors, which `ui::outline_badge` fixes to the foreground.
pub(super) fn badge(
    (border, background, foreground): (Hsla, Hsla, Hsla),
    label: impl Into<SharedString>,
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
            ui::text(label, 10., 14.)
                .flex_none()
                .whitespace_nowrap()
                .font_weight(FontWeight::MEDIUM)
                .text_color(foreground),
        )
}

/// `Badge variant="outline"` with the pages' tinted status classes
/// (`border-*-500/40 bg-*-500/10 text-*-300`).
pub(super) fn tinted_badge(tone: ui::Tone, label: impl Into<SharedString>) -> Div {
    let colors = match tone {
        ui::Tone::Success => (
            Palette::emerald500(0.4),
            Palette::emerald500(0.1),
            Palette::emerald300(),
        ),
        ui::Tone::Warning => (
            Palette::amber500(0.4),
            Palette::amber500(0.1),
            Palette::amber300(),
        ),
        ui::Tone::Error => (
            Tokens::destructive().opacity(0.4),
            Tokens::destructive().opacity(0.1),
            Tokens::destructive(),
        ),
        ui::Tone::Neutral => (
            Tokens::border(),
            Tokens::input().opacity(0.3),
            Tokens::foreground(),
        ),
    };
    badge(colors, label)
}

/// `Button size="sm"` in a variant, with the given label.
pub(super) fn sm_button(variant: controls::Variant, label: &str) -> Div {
    controls::button(variant, 24., false).child(label.to_owned())
}

/// The rows a page's top item draws: card rows `gap-3` apart, or the start of
/// the bordered list under its `header`. `closes` ends the list's border.
pub(super) fn rows_block(mode: ViewMode, header: Div, rows: Vec<AnyElement>, closes: bool) -> Div {
    match mode {
        ViewMode::Cards => div().flex().flex_col().gap(px(CARD_GAP)).children(rows),
        ViewMode::List => div()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded_t(px(10.))
            .when(closes, |table| table.rounded_b(px(10.)))
            .border_1()
            .when(!closes, |table| table.border_b_0())
            .border_color(Tokens::border())
            .child(header)
            .children({
                let count = rows.len();
                rows.into_iter().enumerate().map(move |(index, row)| {
                    div()
                        .when(!closes || index + 1 < count, |row| {
                            row.border_b_1().border_color(Tokens::border())
                        })
                        .child(row)
                })
            }),
    }
}

/// Resolves `grid-template-columns` of `(minimum, fr)` tracks (`fr == 0` is a
/// fixed track) inside a `gap-3 px-3` row at least `min_width` wide. Flexible
/// tracks whose share falls below their minimum freeze there, as CSS does.
pub(super) fn tracks<const N: usize>(
    tracks: [(f32, f32); N],
    min_width: f32,
    width: f32,
) -> [f32; N] {
    let mut free = width.max(min_width) - 24. - (N as f32 - 1.) * CARD_GAP;
    let mut frozen = [false; N];
    for (index, (min, flex)) in tracks.iter().enumerate() {
        if *flex == 0. {
            free -= min;
            frozen[index] = true;
        }
    }
    loop {
        let flex: f32 = (0..N).filter(|i| !frozen[*i]).map(|i| tracks[i].1).sum();
        let unit = if flex > 0. { free / flex } else { 0. };
        let Some(index) = (0..N).find(|i| !frozen[*i] && tracks[*i].1 * unit < tracks[*i].0) else {
            return std::array::from_fn(|i| {
                if frozen[i] {
                    tracks[i].0
                } else {
                    tracks[i].1 * unit
                }
            });
        };
        frozen[index] = true;
        free -= tracks[index].0;
    }
}

/// A row after the top item: `gap-3` above a card row, or the list's side
/// borders, closed at the bottom by the last row.
pub(super) fn continuation(mode: ViewMode, body: Div, last: bool, gap: f32) -> Div {
    match mode {
        ViewMode::Cards => div().flex_1().min_w_0().pt(px(gap)).child(body),
        ViewMode::List => div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .border_x_1()
            .border_b_1()
            .border_color(Tokens::border())
            .when(last, |row| row.rounded_b(px(10.)))
            .child(body),
    }
}
