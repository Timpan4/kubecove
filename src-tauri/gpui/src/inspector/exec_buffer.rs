//! Cache the viewport, never the retained scrollback, for GPUI text rendering.
use gpui_kit::{
    FontStyle, FontWeight, HighlightStyle, Hsla, SharedString, UnderlineStyle, px, rgb,
};
use std::ops::Range;

pub(super) struct TerminalRow {
    pub text: SharedString,
    pub highlights: Vec<(Range<usize>, HighlightStyle)>,
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
    if cell.inverse() || cursor {
        std::mem::swap(&mut foreground, &mut background);
    }
    // xterm's renderer applies DIM_OPACITY=0.5 for SGR faint.
    if cell.dim() {
        foreground = foreground.opacity(0.5);
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
pub(super) fn snapshot(screen: &vt100::Screen) -> Vec<TerminalRow> {
    let (rows, cols) = screen.size();
    let cursor =
        (!screen.hide_cursor() && screen.scrollback() == 0).then(|| screen.cursor_position());
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
                let style = style(cell, cursor == Some((row, col)));
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
            }
        })
        .collect()
}
