//! Buffer coordinates remain stable when the viewport scrolls or receives output.
use super::super::Inspector;
use super::state::ExecTab;
use gpui_kit::{Bounds, ClipboardItem, Context, Pixels, Point};

pub(super) type Cell = (usize, u16);
/// xterm `SelectionModel`: a clicked word or line stays selected while the
/// drag end moves before or after it.
#[derive(Clone, Copy)]
pub(super) struct Selection {
    pub anchor: Cell,
    /// End of the clicked word or line; `anchor` for a character selection.
    pub anchor_end: Cell,
    pub end: Cell,
    /// Alt-drag selects a rectangle, xterm's `SelectionMode.COLUMN`.
    pub column: bool,
}
impl Selection {
    pub fn at(cell: Cell, column: bool) -> Self {
        Self {
            anchor: cell,
            anchor_end: cell,
            end: cell,
            column,
        }
    }
    pub fn ordered(self) -> (Cell, Cell) {
        if self.end < self.anchor {
            (self.end, self.anchor_end)
        } else {
            (self.anchor, self.end.max(self.anchor_end))
        }
    }
    fn columns(self) -> (u16, u16) {
        (self.anchor.1.min(self.end.1), self.anchor.1.max(self.end.1))
    }
    pub fn contains(self, cell: Cell) -> bool {
        let (start, end) = self.ordered();
        if self.column {
            let (left, right) = self.columns();
            (start.0..=end.0).contains(&cell.0) && (left..right).contains(&cell.1)
        } else {
            start <= cell && cell < end
        }
    }
}
impl ExecTab {
    pub(super) fn viewport_start(&self) -> usize {
        self.parser.callbacks().discarded + self.parser.screen().scrollback_len()
            - self.parser.screen().scrollback()
    }
    /// The selection that `selected_text` copies, or `None` when it copies nothing.
    fn copyable_selection(&self) -> Option<Selection> {
        let selection = self.selection?;
        let (start, end) = selection.ordered();
        // xterm ignores a zero-width rectangle.
        let zero_width_rectangle =
            selection.column && selection.columns().0 == selection.columns().1;
        (start != end && !zero_width_rectangle && start.0 >= self.parser.callbacks().discarded)
            .then_some(selection)
    }
    pub(super) fn has_selection(&self) -> bool {
        self.copyable_selection().is_some()
    }
    /// xterm `selectAll`: the whole retained buffer, scrollback included.
    pub(super) fn select_all(&mut self) {
        let first = self.parser.callbacks().discarded;
        let (rows, cols) = self.parser.screen().size();
        let last = first + self.parser.screen().scrollback_len() + usize::from(rows.get()) - 1;
        self.selection = Some(Selection {
            anchor: (first, 0),
            anchor_end: (first, 0),
            end: (last, cols.get()),
            column: false,
        });
        self.refresh_view();
    }
    pub(super) fn selected_text(&mut self) -> Option<String> {
        let selection = self.copyable_selection()?;
        let (start, end) = selection.ordered();
        let columns = selection.column.then(|| selection.columns());
        let offset = self.parser.screen().scrollback();
        let base = self.parser.callbacks().discarded + self.parser.screen().scrollback_len();
        let cols = self.parser.screen().size().1.get();
        let mut text = String::new();
        for index in start.0..=end.0 {
            let row = if index < base {
                self.parser.screen_mut().set_scrollback(base - index);
                0
            } else {
                self.parser.screen_mut().set_scrollback(0);
                u16::try_from(index - base).ok()?
            };
            let (from, to) = columns.unwrap_or((
                if index == start.0 { start.1 } else { 0 },
                if index == end.0 { end.1 } else { cols },
            ));
            let screen = self.parser.screen();
            let line = screen.contents_between(row, from, row, to);
            text.push_str(&line.replace('\u{a0}', " "));
            // Rectangles break every row; xterm joins only soft-wrapped rows.
            if index < end.0
                && (columns.is_some()
                    || (!screen.row_wrapped(row) && !self.parser.wraps.contains_key(&index)))
            {
                text.push_str(if cfg!(target_os = "windows") {
                    "\r\n"
                } else {
                    "\n"
                });
            }
        }
        self.parser.screen_mut().set_scrollback(offset);
        Some(text)
    }
    pub(super) fn word(&self, cell: Cell) -> Selection {
        // xterm DEFAULT_OPTIONS.wordSeparator; wraps join one logical word.
        const SEPARATORS: &str = " ()[]{}'\",`";
        let screen = self.parser.screen();
        let top = self.viewport_start();
        let rows = screen.size().0.get();
        let cols = screen.size().1.get();
        let wrap_end = |row| {
            self.parser
                .wraps
                .get(&(top + usize::from(row)))
                .copied()
                .unwrap_or(cols)
        };
        let wraps = |row| {
            screen.row_wrapped(row) || self.parser.wraps.contains_key(&(top + usize::from(row)))
        };
        let separator = |(row, col)| {
            screen.cell(row, col).is_none_or(|c| {
                !c.is_wide_continuation()
                    && (c.contents().is_empty()
                        || c.contents().chars().any(|c| SEPARATORS.contains(c)))
            })
        };
        let mut start = (u16::try_from(cell.0 - top).unwrap(), cell.1.min(cols - 1));
        if screen
            .cell(start.0, start.1)
            .is_some_and(vt100::Cell::is_wide_continuation)
        {
            start.1 -= 1;
        }
        let class = separator(start);
        let mut end = (start.0, start.1 + 1);
        loop {
            let prev = if start.1 > 0 {
                (start.0, start.1 - 1)
            } else if start.0 > 0 && wraps(start.0 - 1) {
                (start.0 - 1, wrap_end(start.0 - 1) - 1)
            } else {
                break;
            };
            if separator(prev) != class {
                break;
            }
            start = prev;
        }
        loop {
            // A click in the gap a wide character left before its wrap starts past
            // the wrap end, so equality alone would never stop the scan.
            if end.1 >= wrap_end(end.0) {
                if end.0 + 1 < rows && wraps(end.0) {
                    end = (end.0 + 1, 0);
                } else {
                    break;
                }
            }
            if separator(end) != class {
                break;
            }
            end.1 += 1;
        }
        let end = (top + usize::from(end.0), end.1);
        Selection {
            anchor: (top + usize::from(start.0), start.1),
            anchor_end: end,
            end,
            column: false,
        }
    }
    /// xterm `_selectLineAt`: the whole wrapped line, without its line break.
    pub(super) fn line(&self, cell: Cell) -> Selection {
        let screen = self.parser.screen();
        let top = self.viewport_start();
        let (rows, cols) = (screen.size().0.get(), screen.size().1.get());
        let wraps = |row: u16| {
            screen.row_wrapped(row) || self.parser.wraps.contains_key(&(top + usize::from(row)))
        };
        let row = u16::try_from(cell.0 - top).unwrap();
        let first = (0..row)
            .rev()
            .take_while(|r| wraps(*r))
            .last()
            .unwrap_or(row);
        let last = (row..rows - 1)
            .take_while(|r| wraps(*r))
            .last()
            .map_or(row, |r| r + 1);
        Selection {
            anchor_end: (top + usize::from(last), cols),
            ..Selection::at((top + usize::from(first), 0), false)
        }
    }
}
/// Pasted text as the shell receives it. A bracketed paste drops ESC, so clipboard
/// text cannot end the paste early with `ESC [201~` and have the shell run the rest.
pub(super) fn paste_payload(text: &str, bracketed: bool) -> String {
    let text = text.replace("\r\n", "\n").replace('\n', "\r");
    if bracketed {
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', ""))
    } else {
        text
    }
}
impl Inspector {
    pub(super) fn exec_cell(&self, point: Point<Pixels>) -> Option<Cell> {
        let tab = self.exec.active()?;
        let Bounds { origin, size } = self.exec.bounds?;
        let cols = tab.parser.screen().size().1.get();
        let rows = tab.parser.screen().size().0.get();
        let col = ((f32::from(point.x - origin.x) - 10.)
            / ((f32::from(size.width) - 20.) / f32::from(cols)))
        .round()
        .clamp(0., f32::from(cols)) as u16;
        let row = ((f32::from(point.y - origin.y) - 8.) / 14.)
            .floor()
            .clamp(0., f32::from(rows - 1)) as usize;
        Some((tab.viewport_start() + row, col))
    }
    /// The viewport cell under `point`, as xterm's `MouseService.getCoords`
    /// reports it outside selection: the cell itself, not the nearest boundary.
    pub(super) fn exec_target(&self, point: Point<Pixels>) -> Option<(u16, u16)> {
        let tab = self.exec.active()?;
        let Bounds { origin, size } = self.exec.bounds?;
        let (rows, cols) = tab.parser.screen().size();
        let col = ((f32::from(point.x - origin.x) - 10.)
            / ((f32::from(size.width) - 20.) / f32::from(cols.get())))
        .ceil()
        .clamp(1., f32::from(cols.get())) as u16;
        let row = ((f32::from(point.y - origin.y) - 8.) / 14.)
            .ceil()
            .clamp(1., f32::from(rows.get())) as u16;
        Some((row - 1, col - 1))
    }
    /// Mouse-protocol motion reports; selection drags use window listeners.
    pub(super) fn report_exec_motion(
        &mut self,
        event: &gpui_kit::MouseMoveEvent,
        cx: &mut Context<Self>,
    ) {
        if self.exec.drag.is_some() || event.modifiers.shift {
            return;
        }
        let (Some(target), Some(tab)) = (self.exec_target(event.position), self.exec.active())
        else {
            return;
        };
        let mode = tab.parser.screen().mouse_protocol_mode();
        if mode == vt100::MouseProtocolMode::AnyMotion
            || (mode == vt100::MouseProtocolMode::ButtonMotion && event.pressed_button.is_some())
        {
            self.exec_mouse(
                target,
                if event.pressed_button.is_some() {
                    32
                } else {
                    35
                },
                false,
                cx,
            );
        }
    }
    pub(super) fn copy_exec(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self
            .exec
            .tabs
            .iter_mut()
            .find(|t| Some(t.key) == self.exec.active)
            .and_then(ExecTab::selected_text)
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }
    pub(super) fn select_all_exec(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self
            .exec
            .tabs
            .iter_mut()
            .find(|t| Some(t.key) == self.exec.active)
        {
            tab.select_all();
        }
        cx.notify();
    }
    pub(super) fn paste_exec(&mut self, text: &str, cx: &mut Context<Self>) {
        let bracketed = self
            .exec
            .active()
            .is_some_and(|t| t.parser.screen().bracketed_paste());
        self.exec_input(paste_payload(text, bracketed), cx);
    }
    /// A mouse-protocol report for the viewport cell `target` from `exec_target`, which
    /// floors the pointer into its cell as xterm's `getMouseReportCoords` does.
    pub(super) fn exec_mouse(
        &mut self,
        target: (u16, u16),
        button: u16,
        release: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.exec.active() else {
            return;
        };
        let screen = tab.parser.screen();
        let mode = screen.mouse_protocol_mode();
        if mode == vt100::MouseProtocolMode::None
            || (release && mode == vt100::MouseProtocolMode::Press)
        {
            return;
        }
        let (row, col) = (target.0 + 1, target.1 + 1);
        let data = match screen.mouse_protocol_encoding() {
            vt100::MouseProtocolEncoding::Sgr => format!(
                "\x1b[<{button};{col};{row}{}",
                if release { 'm' } else { 'M' }
            ),
            // createExecTerminal subscribes only to onData. xterm routes
            // DEFAULT mouse reports to onBinary, which the web does not forward.
            vt100::MouseProtocolEncoding::Default | vt100::MouseProtocolEncoding::Utf8 => return,
        };
        self.exec_input(data, cx);
    }
}
