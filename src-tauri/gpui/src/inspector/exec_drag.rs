//! Terminal selection gestures from xterm 6 `SelectionService`, which the web
//! terminal uses with the options in `execTerminal.ts`.
use super::{Inspector, exec_selection::Selection};
use gpui_kit::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Task, Window,
};
use std::time::{Duration, Instant};

// xterm `SelectionService`: DRAG_SCROLL_MAX_THRESHOLD, DRAG_SCROLL_MAX_SPEED,
// DRAG_SCROLL_INTERVAL and ALT_CLICK_MOVE_CURSOR_TIME.
const SCROLL_THRESHOLD: f32 = 50.;
const SCROLL_SPEED: f32 = 15.;
const SCROLL_INTERVAL: Duration = Duration::from_millis(50);
const ALT_CLICK_TIME: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Unit {
    Cell,
    Word,
    Line,
}
pub(super) struct Drag {
    unit: Unit,
    /// Rows per interval while the pointer is above (negative) or below the rows.
    scroll: isize,
    down_at: Instant,
    _scroll_task: Task<()>,
}

impl Inspector {
    pub(super) fn select_exec(
        &mut self,
        event: &MouseDownEvent,
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
        // `shouldColumnSelect`: Alt, since macOptionClickForcesSelection is false.
        let (selection, unit) = match event.click_count {
            2 => (tab.word(cell), Unit::Word),
            3.. => (tab.line(cell), Unit::Line),
            _ => (Selection::at(cell, event.modifiers.alt), Unit::Cell),
        };
        tab.selection = Some(selection);
        tab.refresh_view();
        self.exec.drag = Some(Drag {
            unit,
            scroll: 0,
            down_at: cx.background_executor().now(),
            _scroll_task: cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(SCROLL_INTERVAL).await;
                    if !this
                        .update(cx, |this, cx| this.drag_scroll_tick(cx))
                        .unwrap_or(false)
                    {
                        break;
                    }
                }
            }),
        });
        if unit != Unit::Cell {
            self.write_primary(cx);
        }
        cx.notify();
    }
    /// `_getMouseEventScrollAmount`, measured from the rows below the top padding.
    fn drag_scroll_amount(&self, position: Point<Pixels>) -> isize {
        let (Some(bounds), Some(tab)) = (self.exec.bounds, self.exec.active()) else {
            return 0;
        };
        let height = f32::from(tab.parser.screen().size().0.get()) * 14.;
        let mut offset = f32::from(position.y - bounds.origin.y) - 8.;
        if (0. ..=height).contains(&offset) {
            return 0;
        }
        if offset > height {
            offset -= height;
        }
        let offset = offset.clamp(-SCROLL_THRESHOLD, SCROLL_THRESHOLD) / SCROLL_THRESHOLD;
        // JavaScript's Math.round rounds halves up.
        (offset.signum() + (offset * (SCROLL_SPEED - 1.) + 0.5).floor()) as isize
    }
    /// `_handleMouseMove`, registered on the window so it follows the pointer
    /// outside the terminal.
    pub(super) fn drag_exec(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(unit) = self.exec.drag.as_ref().map(|d| d.unit) else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            // The button was released where the window did not see it.
            self.exec.drag = None;
            return;
        }
        let scroll = self.drag_scroll_amount(event.position);
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
        let Some(selection) = tab.selection else {
            return;
        };
        let cols = tab.parser.screen().size().1.get();
        match unit {
            Unit::Line => cell.1 = if cell.0 < selection.anchor.0 { 0 } else { cols },
            Unit::Word => {
                let word = tab.word(cell);
                cell = if cell < selection.anchor {
                    word.anchor
                } else {
                    word.end
                };
            }
            Unit::Cell => {}
        }
        if !selection.column && scroll != 0 {
            cell.1 = if scroll > 0 { cols } else { 0 };
        }
        let row = u16::try_from(cell.0 - tab.viewport_start()).unwrap();
        if tab
            .parser
            .screen()
            .cell(row, cell.1)
            .is_some_and(vt100::Cell::is_wide_continuation)
        {
            if cell >= selection.anchor {
                cell.1 += 1;
            } else {
                cell.1 -= 1;
            }
        }
        tab.selection = Some(Selection {
            end: cell,
            ..selection
        });
        tab.refresh_view();
        if let Some(drag) = &mut self.exec.drag {
            drag.scroll = scroll;
        }
        cx.notify();
    }
    /// `_dragScroll`: scroll, then extend the selection to the exposed edge.
    fn drag_scroll_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(scroll) = self.exec.drag.as_ref().map(|d| d.scroll) else {
            return false;
        };
        if scroll == 0 {
            return true;
        }
        let Some(tab) = self
            .exec
            .tabs
            .iter_mut()
            .find(|t| Some(t.key) == self.exec.active)
        else {
            return false;
        };
        let offset = tab.parser.screen().scrollback();
        tab.parser
            .screen_mut()
            .set_scrollback(offset.saturating_add_signed(-scroll));
        let (rows, cols) = tab.parser.screen().size();
        let top = tab.viewport_start();
        let last = tab.parser.callbacks().discarded
            + tab.parser.screen().scrollback_len()
            + usize::from(rows.get())
            - 1;
        if let Some(selection) = &mut tab.selection {
            selection.end = if scroll > 0 {
                (
                    (top + usize::from(rows.get())).min(last),
                    if selection.column {
                        selection.end.1
                    } else {
                        cols.get()
                    },
                )
            } else {
                (top, if selection.column { selection.end.1 } else { 0 })
            };
        }
        tab.refresh_view();
        cx.notify();
        true
    }
    /// `_handleMouseUp`: an Alt click without a selection moves the shell cursor.
    pub(super) fn end_exec_drag(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        let Some(drag) = self.exec.drag.take() else {
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
        let length = tab
            .selected_text()
            .map_or(0, |text| text.encode_utf16().count());
        let quick = cx.background_executor().now() - drag.down_at < ALT_CLICK_TIME;
        // altClickMovesCursor defaults to true and `execTerminal.ts` keeps it.
        if length <= 1 && quick && event.modifiers.alt {
            if tab.parser.screen().scrollback() == 0
                && let Some(target) = self.exec_target(event.position)
                && let Some(tab) = self.exec.active()
            {
                let sequence = super::exec_move::sequence(tab.parser.screen(), target);
                self.exec_input(sequence, cx);
            }
        } else {
            self.write_primary(cx);
        }
    }
    /// xterm `onLinuxMouseSelection` places a mouse selection in the primary
    /// selection, which WebKitGTK pastes on middle click.
    fn write_primary(&mut self, cx: &mut Context<Self>) {
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        if let Some(text) = self
            .exec
            .tabs
            .iter_mut()
            .find(|t| Some(t.key) == self.exec.active)
            .and_then(super::exec_state::ExecTab::selected_text)
        {
            cx.write_to_primary(gpui_kit::ClipboardItem::new_string(text));
        }
        #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
        let _ = cx;
    }
}

/// Middle click pastes the primary selection on Linux, as xterm's `auxclick`
/// handler lets WebKitGTK paste it into the terminal's textarea. Mouse-protocol
/// applications receive the click instead, unless Shift forces selection.
pub(super) fn middle_paste<E: gpui_kit::InteractiveElement>(
    element: E,
    cx: &mut Context<Inspector>,
) -> E {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    return element.on_mouse_down(
        MouseButton::Middle,
        cx.listener(|this, event: &MouseDownEvent, window, cx| {
            let reporting = this.exec.active().is_some_and(|tab| {
                tab.parser.screen().mouse_protocol_mode() != vt100::MouseProtocolMode::None
            });
            if (reporting && !event.modifiers.shift) || this.exec.active().is_none() {
                return;
            }
            window.focus(&this.exec.focus, cx);
            if let Some(text) = cx.read_from_primary().and_then(|item| item.text()) {
                this.paste_exec(&text, cx);
            }
        }),
    );
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        let _ = cx;
        element
    }
}
