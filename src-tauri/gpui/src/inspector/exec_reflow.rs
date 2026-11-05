//! Reflow retained logical lines on fit, outside the frame rendering path.
use std::{fmt::Write, num::NonZeroU16};

#[derive(Default)]
pub(super) struct History {
    pub discarded: usize,
}
impl vt100::Callbacks for History {
    fn on_scroll(&mut self, _: vt100::capture::RowContents<'_>, alternate: bool) {
        if !alternate {
            self.discarded += 1;
        }
    }
}
pub(super) struct Parser {
    inner: vt100::Parser<History>,
    pub wraps: std::collections::BTreeMap<usize, u16>,
    erase_scan: EraseScan,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum EraseScan {
    #[default]
    Ground,
    Escape,
    Csi,
    Display,
    History,
}
impl std::ops::Deref for Parser {
    type Target = vt100::Parser<History>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl std::ops::DerefMut for Parser {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl Parser {
    pub fn new_with_callbacks(
        rows: NonZeroU16,
        cols: NonZeroU16,
        capacity: usize,
        callbacks: History,
    ) -> Self {
        Self {
            inner: vt100::Parser::new_with_callbacks(rows, cols, capacity, callbacks),
            wraps: Default::default(),
            erase_scan: EraseScan::Ground,
        }
    }
    pub fn process(&mut self, bytes: &[u8]) {
        if bytes.is_ascii() && !bytes.contains(&0x1b) && self.erase_scan == EraseScan::Ground {
            self.inner.process(bytes);
            let discarded = self.inner.callbacks().discarded;
            while self
                .wraps
                .first_key_value()
                .is_some_and(|(row, _)| *row < discarded)
            {
                self.wraps.pop_first();
            }
            return;
        }
        for byte in bytes {
            match (self.erase_scan, *byte) {
                (EraseScan::Escape, b'c') | (EraseScan::History, b'J') => self.wraps.clear(),
                (EraseScan::Display, b'J') => {
                    let base =
                        self.inner.callbacks().discarded + self.inner.screen().scrollback_len();
                    self.wraps.retain(|row, _| *row < base);
                }
                _ => {}
            }
            self.erase_scan = match (self.erase_scan, *byte) {
                (_, 0x1b) => EraseScan::Escape,
                (EraseScan::Escape, b'[') => EraseScan::Csi,
                (EraseScan::Csi, b'2') => EraseScan::Display,
                (EraseScan::Csi, b'3') => EraseScan::History,
                _ => EraseScan::Ground,
            };
            let screen = self.inner.screen();
            let (row, col) = screen.cursor_position();
            let cols = screen.size().1.get();
            let absolute =
                self.inner.callbacks().discarded + screen.scrollback_len() + usize::from(row);
            let gap = !screen.alternate_screen() && col == cols - 1;
            self.inner.process(std::slice::from_ref(byte));
            let screen = self.inner.screen();
            let (next_row, next_col) = screen.cursor_position();
            let next_absolute =
                self.inner.callbacks().discarded + screen.scrollback_len() + usize::from(next_row);
            // vt100 intentionally follows tmux here. xterm joins a wide glyph
            // moved past an unused final column to the preceding logical line.
            if gap
                && next_absolute == absolute + 1
                && next_col == 2
                && screen.cell(next_row, 0).is_some_and(vt100::Cell::is_wide)
            {
                self.wraps.insert(absolute, cols - 1);
            }
            let discarded = self.inner.callbacks().discarded;
            while self
                .wraps
                .first_key_value()
                .is_some_and(|(row, _)| *row < discarded)
            {
                self.wraps.pop_first();
            }
        }
    }
}

fn color(output: &mut String, color: vt100::Color, foreground: bool) {
    let prefix = if foreground { 38 } else { 48 };
    match color {
        vt100::Color::Default => {}
        vt100::Color::Idx(index) => write!(output, ";{prefix};5;{index}").unwrap(),
        vt100::Color::Rgb(r, g, b) => write!(output, ";{prefix};2;{r};{g};{b}").unwrap(),
    }
}
fn cell(output: &mut String, cell: &vt100::Cell) {
    output.clear();
    output.push_str("\x1b[0");
    for (enabled, code) in [
        (cell.bold(), 1),
        (cell.dim(), 2),
        (cell.italic(), 3),
        (cell.underline(), 4),
        (cell.inverse(), 7),
    ] {
        if enabled {
            write!(output, ";{code}").unwrap();
        }
    }
    color(output, cell.fgcolor(), true);
    color(output, cell.bgcolor(), false);
    output.push('m');
    output.push_str(if cell.contents().is_empty() {
        " "
    } else {
        cell.contents()
    });
}
fn position(parser: &Parser) -> (usize, u16) {
    let screen = parser.screen();
    let (row, col) = screen.cursor_position();
    (
        parser.callbacks().discarded + screen.scrollback_len() + usize::from(row),
        col,
    )
}

pub(super) fn resize(parser: &mut Parser, rows: NonZeroU16, cols: NonZeroU16) {
    let screen = parser.screen();
    // Full-screen applications own their layout and redraw after SIGWINCH.
    // Like xterm's alternate buffer, these rows must not enter normal history.
    if screen.alternate_screen() || screen.size().1 == cols {
        parser.set_size(rows, cols);
        return;
    }
    let mut old = screen.clone();
    old.set_scrollback(0);
    let (cursor_row, cursor_col) = old.cursor_position();
    let history = old.scrollback_len();
    let old_cols = old.size().1.get();
    let old_base = parser.callbacks().discarded;
    let mut cursor_start = history + usize::from(cursor_row);
    while cursor_start > 0 {
        let index = cursor_start - 1;
        let row = if index < history {
            old.set_scrollback(history - index);
            0
        } else {
            old.set_scrollback(0);
            u16::try_from(index - history).unwrap()
        };
        if !old.row_wrapped(row) && !parser.wraps.contains_key(&(old_base + index)) {
            break;
        }
        cursor_start -= 1;
    }
    old.set_scrollback(0);
    let mut cursor_end = cursor_row;
    while cursor_end + 1 < old.size().0.get()
        && (old.row_wrapped(cursor_end)
            || parser
                .wraps
                .contains_key(&(old_base + history + usize::from(cursor_end))))
    {
        cursor_end += 1;
    }
    let last = (0..old.size().0.get())
        .rev()
        .find(|row| {
            *row <= cursor_row
                || (0..old_cols).any(|col| {
                    old.cell(*row, col)
                        .is_some_and(|c| c.has_contents() || c.bgcolor() != vt100::Color::Default)
                })
        })
        .unwrap_or(cursor_row);
    let mut next =
        Parser::new_with_callbacks(rows, cols, old.scrollback_capacity(), History::default());
    let mut encoded = String::new();
    let mut cursor = (0, 0);
    for index in 0..=history + usize::from(last) {
        let row = if index < history {
            old.set_scrollback(history - index);
            0
        } else {
            old.set_scrollback(0);
            u16::try_from(index - history).unwrap()
        };
        let override_end = parser.wraps.get(&(old_base + index)).copied();
        let wrapped = old.row_wrapped(row) || override_end.is_some();
        // xterm's reflowCursorLine default is false. The remote program owns
        // the physical rows of the current input line after SIGWINCH.
        let cursor_line = index >= cursor_start && index <= history + usize::from(cursor_end);
        let next_row = position(&next).0;
        let cursor_here = index == history + usize::from(cursor_row);
        let mut end = if wrapped {
            override_end.unwrap_or(old_cols)
        } else {
            (0..old_cols)
                .rev()
                .find(|col| {
                    old.cell(row, *col)
                        .is_some_and(|c| c.has_contents() || c.bgcolor() != vt100::Color::Default)
                })
                .map_or(0, |col| col + 1)
                .max(if cursor_here { cursor_col } else { 0 })
        };
        if cursor_line {
            end = end.min(cols.get());
        }
        for col in 0..=end {
            if cursor_here && col == cursor_col.min(end) {
                cursor = position(&next);
            }
            if col == end {
                break;
            }
            let Some(value) = old.cell(row, col) else {
                continue;
            };
            if value.is_wide_continuation() || (cursor_line && value.is_wide() && col + 1 >= end) {
                continue;
            }
            cell(&mut encoded, value);
            next.process(encoded.as_bytes());
        }
        if cursor_line && wrapped {
            next.wraps.insert(next_row, end);
        }
        if (!wrapped || cursor_line) && index < history + usize::from(last) {
            next.process(b"\r\n");
        }
    }
    let base = next.callbacks().discarded + next.screen().scrollback_len();
    let row = cursor
        .0
        .saturating_sub(base)
        .min(usize::from(rows.get() - 1));
    next.process(format!("\x1b[{};{}H", row + 1, cursor.1.min(cols.get() - 1) + 1).as_bytes());
    next.process(screen.input_mode_formatted().as_bytes());
    next.process(screen.attributes_formatted().as_bytes());
    if screen.hide_cursor() {
        next.process(b"\x1b[?25l");
    }
    // Preserve the VTE decoder, including an incomplete UTF-8 or escape sequence.
    *parser.screen_mut() = next.screen().clone();
    let discarded = next.callbacks().discarded;
    parser.wraps = next
        .wraps
        .iter()
        .map(|(row, end)| (row - discarded, *end))
        .collect();
    parser.callbacks_mut().discarded = 0;
}
