//! Cache the viewport, never the retained scrollback, for GPUI text rendering.
use gpui_kit::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, Hsla, SharedString, StyledText,
    TestSupportExt, UnderlineStyle, div, prelude::*, px, rgb,
};
use std::ops::Range;

pub(super) struct TerminalRow {
    pub text: SharedString,
    pub highlights: Vec<(Range<usize>, HighlightStyle)>,
    /// The cursor cell's byte range and its block-cursor style.
    pub cursor: Option<(Range<usize>, HighlightStyle)>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CursorShape {
    Block,
    Bar,
    Underline,
}
/// How the cursor cell paints in this frame, following xterm's
/// `DomRendererRowFactory`: focused terminals use the shape and may blink,
/// unfocused ones use `cursorInactiveStyle`, whose default is an outline.
#[derive(Clone, Copy)]
pub(super) struct CursorPaint {
    pub shape: CursorShape,
    pub focused: bool,
    pub blink: bool,
    /// False during the hidden half of xterm's 1s step-end blink animation.
    pub phase: bool,
}
/// DECSCUSR shape and blink. The default is `execTerminal.ts`: xterm's block
/// cursorStyle with cursorBlink enabled.
pub(super) fn cursor_shape(style: vt100::CursorStyle) -> (CursorShape, bool) {
    match style {
        vt100::CursorStyle::Default | vt100::CursorStyle::BlinkBlock => (CursorShape::Block, true),
        vt100::CursorStyle::SteadyBlock => (CursorShape::Block, false),
        vt100::CursorStyle::BlinkUnderline => (CursorShape::Underline, true),
        vt100::CursorStyle::SteadyUnderline => (CursorShape::Underline, false),
        vt100::CursorStyle::BlinkVerticalBar => (CursorShape::Bar, true),
        vt100::CursorStyle::SteadyVerticalBar => (CursorShape::Bar, false),
    }
}
impl CursorPaint {
    fn label(self) -> &'static str {
        match (self.focused, self.shape, self.blink) {
            (false, ..) => "outline",
            (true, CursorShape::Block, true) => "block blink",
            (true, CursorShape::Block, false) => "block",
            (true, CursorShape::Bar, true) => "bar blink",
            (true, CursorShape::Bar, false) => "bar",
            (true, CursorShape::Underline, true) => "underline blink",
            (true, CursorShape::Underline, false) => "underline",
        }
    }
}
fn slice(row: &TerminalRow, range: Range<usize>) -> StyledText {
    let highlights = row
        .highlights
        .iter()
        .filter(|(r, _)| r.start < range.end && r.end > range.start)
        .map(|(r, style)| {
            (
                r.start.max(range.start) - range.start..r.end.min(range.end) - range.start,
                *style,
            )
        })
        .collect::<Vec<_>>();
    StyledText::new(SharedString::from(row.text[range].to_owned())).with_highlights(highlights)
}
pub(super) fn row_element(row: &TerminalRow, paint: CursorPaint) -> AnyElement {
    let line = div().h(px(14.)).whitespace_nowrap();
    let Some((range, block)) = &row.cursor else {
        return line
            .child(
                StyledText::new(row.text.clone()).with_highlights(row.highlights.iter().cloned()),
            )
            .into_any_element();
    };
    let shown = !paint.focused || !paint.blink || paint.phase;
    let cell = if shown && paint.focused && paint.shape == CursorShape::Block {
        StyledText::new(SharedString::from(row.text[range.clone()].to_owned()))
            .with_highlights([(0..range.len(), *block)])
    } else {
        slice(row, range.clone())
    };
    // xterm DomRenderer cursor CSS: bar is a cursorWidth=1px inset box-shadow,
    // underline a 1px bottom border, the inactive outline a 1px outline at -1px.
    let mark = div().absolute().bg(rgb(0xffffff));
    let overlay = match (paint.focused, paint.shape) {
        _ if !shown => None,
        (false, _) => Some(
            div()
                .absolute()
                .inset_0()
                .border_1()
                .border_color(rgb(0xffffff)),
        ),
        (true, CursorShape::Bar) => Some(mark.top_0().left_0().bottom_0().w(px(1.))),
        (true, CursorShape::Underline) => Some(mark.left_0().right_0().bottom_0().h(px(1.))),
        (true, CursorShape::Block) => None,
    };
    line.flex()
        .child(slice(row, 0..range.start))
        .child(
            div()
                .id("exec-cursor")
                .test_support()
                .aria_label(paint.label())
                .relative()
                .child(cell)
                .children(overlay),
        )
        .child(slice(row, range.end..row.text.len()))
        .into_any_element()
}

// xterm.js `src/browser/Types.ts::DEFAULT_ANSI_COLORS`. The web leaves this
// palette at its default. Extended colors use the xterm 6x6x6 cube and gray ramp.
const ANSI: [u32; 16] = [
    0x2e3436, 0xcc0000, 0x4e9a06, 0xc4a000, 0x3465a4, 0x75507b, 0x06989a, 0xd3d7cf, 0x555753,
    0xef2929, 0x8ae234, 0xfce94f, 0x729fcf, 0xad7fa8, 0x34e2e2, 0xeeeeec,
];
fn color(value: vt100::Color, default: u32) -> Hsla {
    let value = match value {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
        vt100::Color::Idx(index) if index < 16 => ANSI[usize::from(index)],
        vt100::Color::Idx(index) if index < 232 => {
            let index = u32::from(index) - 16;
            let levels = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
            levels[(index / 36) as usize] << 16
                | levels[((index / 6) % 6) as usize] << 8
                | levels[(index % 6) as usize]
        }
        vt100::Color::Idx(index) => {
            let value = 8 + (u32::from(index) - 232) * 10;
            value << 16 | value << 8 | value
        }
    };
    rgb(value).into()
}
fn style(cell: &vt100::Cell, cursor: bool) -> HighlightStyle {
    let foreground = match cell.fgcolor() {
        vt100::Color::Idx(i) if cell.bold() && i < 8 => vt100::Color::Idx(i + 8),
        other => other,
    };
    let (mut foreground, mut background) =
        (color(foreground, 0xf4f4f5), color(cell.bgcolor(), 0x101113));
    if cell.inverse() {
        std::mem::swap(&mut foreground, &mut background);
    }
    if cursor {
        foreground = rgb(0x000000).into();
        background = rgb(0xffffff).into();
    }
    // xterm's renderer applies DIM_OPACITY=0.5 for SGR faint.
    if cell.dim() {
        // HighlightStyle blends with the inherited text color, so resolve
        // opacity against the terminal background before applying the run.
        foreground = background.blend(foreground.opacity(0.5));
    }
    HighlightStyle {
        color: Some(foreground),
        background_color: Some(background),
        font_weight: cell.bold().then_some(FontWeight::BOLD),
        font_style: cell.italic().then_some(FontStyle::Italic),
        underline: cell.underline().then_some(UnderlineStyle {
            thickness: px(1.),
            color: Some(foreground),
            wavy: false,
        }),
        ..Default::default()
    }
}
pub(super) fn snapshot(
    screen: &vt100::Screen,
    selection: Option<super::exec_selection::Selection>,
    top: usize,
) -> Vec<TerminalRow> {
    let (rows, cols) = screen.size();
    let cursor = (!screen.hide_cursor() && screen.scrollback() == 0).then(|| {
        let (row, col) = screen.cursor_position();
        let col = col.min(cols.get() - 1);
        let continuation = screen
            .cell(row, col)
            .is_some_and(vt100::Cell::is_wide_continuation);
        (row, col - u16::from(continuation && col > 0))
    });
    (0..rows.get())
        .map(|row| {
            let end = (0..cols.get())
                .rev()
                .find(|col| {
                    screen.cell(row, *col).is_some_and(|cell| {
                        !cell.contents().is_empty()
                            || cell.bgcolor() != vt100::Color::Default
                            || cursor == Some((row, *col))
                    })
                })
                .map_or(0, |col| col + 1);
            let mut text = String::new();
            let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
            let mut cursor_cell = None;
            for col in 0..end {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };
                if cell.is_wide_continuation() {
                    continue;
                }
                let start = text.len();
                if cell.contents().is_empty() {
                    text.push(' ');
                } else {
                    text.push_str(cell.contents());
                }
                if cursor == Some((row, col)) {
                    cursor_cell = Some((start..text.len(), style(cell, true)));
                }
                let mut style = style(cell, false);
                if selection.is_some_and(|s| s.contains((top + usize::from(row), col))) {
                    // ThemeService.DEFAULT_SELECTION, white at 30% opacity.
                    style.background_color = Some(
                        color(vt100::Color::Default, 0x101113)
                            .blend(gpui_kit::Hsla::from(rgb(0xffffff)).opacity(0.3)),
                    );
                }
                if let Some((range, previous)) = highlights.last_mut()
                    && *previous == style
                {
                    range.end = text.len();
                } else {
                    highlights.push((start..text.len(), style));
                }
            }
            TerminalRow {
                text: text.into(),
                highlights,
                cursor: cursor_cell,
            }
        })
        .collect()
}
