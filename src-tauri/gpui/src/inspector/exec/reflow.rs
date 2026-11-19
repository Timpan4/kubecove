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
    /// xterm `Terminal.clear()`: drop the history and every row except the
    /// cursor's, which becomes row 0. The cursor keeps its column.
    pub fn clear(&mut self) {
        // Read rows as drawn, not as scrolled; xterm also returns to the bottom.
        self.inner.screen_mut().set_scrollback(0);
        let screen = self.inner.screen();
        let (row, col) = screen.cursor_position();
        let history = screen.scrollback_len();
        if history == 0 && row == 0 {
            return;
        }
        let (rows, cols) = screen.size();
        let margins = scroll_region(screen);
        let mut bytes = String::new();
        if margins.is_some() {
            // Scroll and line edits need the full region. DECSTBM leaves DECOM set.
            bytes.push_str("\x1b[r");
        }
        if row > 0 {
            write!(bytes, "\x1b[{row}S").unwrap();
        }
        let below = rows.get() - 1;
        if below > 0 {
            if screen.row_wrapped(row) {
                // vt100 clears the bottom row's wrap flag on insert. Move the
                // retained row there, then delete the inserted rows to bring it back.
                write!(bytes, "\x1b[1d\x1b[{below}L").unwrap();
            } else {
                bytes.push_str("\x1b[2d");
            }
            write!(bytes, "\x1b[{below}M").unwrap();
        }
        if let Some((top, bottom)) = margins {
            write!(bytes, "\x1b[{top};{bottom}r").unwrap();
            if col < cols.get() {
                write!(bytes, "\x1b[{}G", col + 1).unwrap();
            }
        }
        let alternate = screen.alternate_screen();
        let target = self.inner.callbacks().discarded + history + usize::from(row);
        // Run the surgery on a scratch parser so a pending escape or UTF-8
        // sequence in the live decoder survives, as in `resize`.
        let mut scratch = vt100::Parser::new(rows, cols, 0);
        std::mem::swap(self.inner.screen_mut(), scratch.screen_mut());
        scratch.process(bytes.as_bytes());
        if margins.is_some() && col == cols.get() {
            restore_pending_wrap(&mut scratch, cols.get());
        }
        // vt100's VPA uses a physical row and keeps the column, including a
        // pending wrap. CUP would honor DECOM and clamp the column.
        scratch.process(b"\x1b[1d");
        scratch.screen_mut().clear_scrollback();
        std::mem::swap(self.inner.screen_mut(), scratch.screen_mut());
        if !alternate {
            // Keep absolute row indices increasing, so indices into the dropped
            // rows fall below `discarded` and stop matching.
            self.inner.callbacks_mut().discarded = target;
            self.wraps.clear();
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

/// DECSTBM homes the cursor. Repeat the last printed character on a blank
/// row to recover the pending column without changing the retained cells or REP.
fn restore_pending_wrap(scratch: &mut vt100::Parser, cols: u16) {
    let attributes = scratch.screen().attributes_formatted();
    scratch.process(b"\x1b[0m\x1b[2d\x1b[b");
    let (_, width) = scratch.screen().cursor_position();
    if (1..cols).contains(&width) {
        scratch.process(format!("\x1b[{}G\x1b[b", cols - width + 1).as_bytes());
    }
    // vt100's EL 2 erases the scratch row without moving the cursor.
    scratch.process(b"\x1b[2K");
    scratch.process(attributes.as_bytes());
}

/// One-based DECSTBM bounds, or `None` for the full screen.
fn scroll_region(screen: &vt100::Screen) -> Option<(u16, u16)> {
    let (rows, cols) = screen.size();
    let mut probe = vt100::Parser::new(rows, cols, 0);
    *probe.screen_mut() = screen.clone();
    // Origin mode makes home land at the top margin and clamps CUP at the bottom.
    probe.process(b"\x1b[?6h");
    let top = probe.screen().cursor_position().0 + 1;
    probe.process(format!("\x1b[{}H", rows.get()).as_bytes());
    let bottom = probe.screen().cursor_position().0 + 1;
    (top != 1 || bottom != rows.get()).then_some((top, bottom))
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
    let (cursor_row, cursor_col) = screen.cursor_position();
    let mut scratch = vt100::Parser::new(rows, cols, 0);
    *scratch.screen_mut() = screen.clone();
    // vt100 has no DECSC accessor, so read the saved cursor and attributes
    // by restoring them on the copy. They are re-saved at the same logical cell.
    scratch.process(b"\x1b8");
    let (saved_row, saved_col) = scratch.screen().cursor_position();
    let saved_attributes = scratch.screen().attributes_formatted();
    let old = scratch.screen_mut();
    old.set_scrollback(0);
    let history = old.scrollback_len();
    let saved_index = history + usize::from(saved_row);
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
    let mut saved = None;
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
                // A wide character's second column has no contents of its own.
                .map_or(0, |col| {
                    let wide = old.cell(row, col).is_some_and(vt100::Cell::is_wide);
                    col + if wide { 2 } else { 1 }
                })
                .max(if cursor_here { cursor_col } else { 0 })
        };
        if cursor_line {
            end = end.min(cols.get());
        }
        // The cursor line drops a wide cell that no longer fits.
        let replayed = |value: &vt100::Cell, col: u16| {
            !value.is_wide_continuation() && !(cursor_line && value.is_wide() && col + 1 >= end)
        };
        for col in 0..=end {
            if cursor_here && col == cursor_col.min(end) {
                cursor = position(&next);
            }
            if index == saved_index && col == saved_col.min(end) {
                // A saved column past the row's content keeps its offset
                // instead of writing blank cells that copy would include.
                // A cell that does not fit, such as a wide character with one
                // column left, starts the next row when it is replayed.
                let width = old
                    .cell(row, col)
                    .filter(|c| col < end && replayed(c, col))
                    .map_or(0, |c| if c.is_wide() { 2 } else { 1 });
                let (mut next_row, mut at) = position(&next);
                if width > 0 && at.saturating_add(width) > cols.get() {
                    (next_row, at) = (next_row + 1, 0);
                } else if col < end
                    && old
                        .cell(row, col)
                        .is_some_and(vt100::Cell::is_wide_continuation)
                {
                    // The wide character is already replayed; its second
                    // column is one cell back.
                    at = at.saturating_sub(1);
                }
                saved = Some((next_row, at.saturating_add(saved_col - col)));
            }
            if col == end {
                break;
            }
            let Some(value) = old.cell(row, col) else {
                continue;
            };
            if !replayed(value, col) {
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
    // A saved row below the last replayed one keeps its distance from it.
    let saved = saved.unwrap_or_else(|| {
        let row = position(&next).0 + (saved_index - history - usize::from(last));
        (row, saved_col)
    });
    let base = next.callbacks().discarded + next.screen().scrollback_len();
    let move_to = |(row, col): (usize, u16)| {
        let row = row.saturating_sub(base).min(usize::from(rows.get() - 1));
        format!("\x1b[{};{}H", row + 1, col.min(cols.get() - 1) + 1)
    };
    next.process(format!("{}{saved_attributes}\x1b7", move_to(saved)).as_bytes());
    next.process(move_to(cursor).as_bytes());
    next.process(screen.input_mode_formatted().as_bytes());
    next.process(screen.attributes_formatted().as_bytes());
    if screen.hide_cursor() {
        next.process(b"\x1b[?25l");
    }
    // DECSCUSR lives on the replaced screen.
    let style = match screen.cursor_style() {
        vt100::CursorStyle::Default => 0,
        vt100::CursorStyle::BlinkBlock => 1,
        vt100::CursorStyle::SteadyBlock => 2,
        vt100::CursorStyle::BlinkUnderline => 3,
        vt100::CursorStyle::SteadyUnderline => 4,
        vt100::CursorStyle::BlinkVerticalBar => 5,
        vt100::CursorStyle::SteadyVerticalBar => 6,
    };
    next.process(format!("\x1b[{style} q").as_bytes());
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
