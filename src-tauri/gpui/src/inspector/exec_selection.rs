//! Buffer coordinates remain stable when the viewport scrolls or receives output.
use super::{Inspector, exec_state::ExecTab};
use gpui_kit::{Bounds, ClipboardItem, Context, Pixels, Point, Window};

pub(super) type Cell = (usize, u16);
#[derive(Clone, Copy)]
pub(super) struct Selection {
    pub anchor: Cell,
    pub end: Cell,
}
impl Selection {
    pub fn ordered(self) -> (Cell, Cell) {
        (self.anchor.min(self.end), self.anchor.max(self.end))
    }
    pub fn contains(self, cell: Cell) -> bool {
        let (start, end) = self.ordered();
        start <= cell && cell < end
    }
}
impl ExecTab {
    pub(super) fn viewport_start(&self) -> usize {
        self.parser.callbacks().discarded + self.parser.screen().scrollback_len()
            - self.parser.screen().scrollback()
    }
    pub(super) fn selected_text(&mut self) -> Option<String> {
        let (start, end) = self.selection?.ordered();
        if start == end || start.0 < self.parser.callbacks().discarded {
            return None;
        }
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
            let from = if index == start.0 { start.1 } else { 0 };
            let to = if index == end.0 { end.1 } else { cols };
            let screen = self.parser.screen();
            let line = screen.contents_between(row, from, row, to);
            text.push_str(&line.replace('\u{a0}', " "));
            if index < end.0 && !screen.row_wrapped(row) && !self.parser.wraps.contains_key(&index)
            {
                text.push(if cfg!(target_os = "windows") {
                    '\r'
                } else {
                    '\n'
                });
                if cfg!(target_os = "windows") {
                    text.push('\n');
                }
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
            if end.1 == wrap_end(end.0) {
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
        Selection {
            anchor: (top + usize::from(start.0), start.1),
            end: (top + usize::from(end.0), end.1),
        }
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
    pub(super) fn select_exec(
        &mut self,
        event: &gpui_kit::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.exec.focus, cx);
        let Some(mut cell) = self.exec_cell(event.position) else {
            return;
        };
        let Some(tab) = self
            .exec
            .tabs
            .iter_mut()
            .find(|t| Some(t.key) == self.exec.active)
        else {
            return;
        };
        if tab.parser.screen().mouse_protocol_mode() != vt100::MouseProtocolMode::None
            && !event.modifiers.shift
        {
            self.exec_mouse(cell, 0, false, cx);
            return;
        }
        let row = u16::try_from(cell.0 - tab.viewport_start()).unwrap();
        if tab
            .parser
            .screen()
            .cell(row, cell.1)
            .is_some_and(vt100::Cell::is_wide_continuation)
        {
            cell.1 -= 1;
        }
        tab.selection = Some(if event.click_count == 2 {
            tab.word(cell)
        } else {
            Selection {
                anchor: cell,
                end: cell,
            }
        });
        self.exec.selecting = event.click_count == 1;
        tab.refresh_view();
        cx.notify();
    }
    pub(super) fn drag_exec(&mut self, event: &gpui_kit::MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(mut cell) = self.exec_cell(event.position) else {
            return;
        };
        let Some(tab) = self
            .exec
            .tabs
            .iter_mut()
            .find(|t| Some(t.key) == self.exec.active)
        else {
            return;
        };
        if self.exec.selecting && event.pressed_button == Some(gpui_kit::MouseButton::Left) {
            let row = u16::try_from(cell.0 - tab.viewport_start()).unwrap();
            let continuation = tab
                .parser
                .screen()
                .cell(row, cell.1)
                .is_some_and(vt100::Cell::is_wide_continuation);
            if let Some(selection) = &mut tab.selection {
                if continuation {
                    if cell >= selection.anchor {
                        cell.1 += 1;
                    } else {
                        cell.1 -= 1;
                    }
                }
                selection.end = cell;
            }
            tab.refresh_view();
            cx.notify();
        } else if !event.modifiers.shift {
            let mode = tab.parser.screen().mouse_protocol_mode();
            if mode == vt100::MouseProtocolMode::AnyMotion
                || (mode == vt100::MouseProtocolMode::ButtonMotion
                    && event.pressed_button.is_some())
            {
                self.exec_mouse(
                    cell,
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
    pub(super) fn paste_exec(&mut self, text: &str, cx: &mut Context<Self>) {
        let mut text = text.replace("\r\n", "\n").replace('\n', "\r");
        if self
            .exec
            .active()
            .is_some_and(|t| t.parser.screen().bracketed_paste())
        {
            text = format!("\x1b[200~{text}\x1b[201~");
        }
        self.exec_input(text, cx);
    }
    pub(super) fn exec_mouse(
        &mut self,
        cell: Cell,
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
        let col = cell.1.min(screen.size().1.get() - 1) + 1;
        let row = cell.0 - tab.viewport_start() + 1;
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
