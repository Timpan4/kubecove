//! xterm's keyboard sequences and platform clipboard gestures.
use super::super::Inspector;
use gpui_kit::{Context, KeyDownEvent};

impl Inspector {
    /// Pastes the clipboard's text, for Shift+Insert and the platform paste gesture.
    /// The terminal's context menu calls this too.
    pub(super) fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.paste_exec(&text, cx);
        }
    }
    pub(super) fn exec_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let m = key.modifiers;
        let clipboard = cfg!(target_os = "macos") && m.platform && !m.control && !m.alt;
        let control_clipboard = !cfg!(target_os = "macos") && m.control && !m.alt && !m.platform;
        if (clipboard && !m.shift && key.key == "c")
            || (control_clipboard && !m.shift && key.key == "insert")
        {
            self.copy_exec(cx);
            cx.stop_propagation();
            return;
        }
        if (clipboard && key.key == "v")
            || (control_clipboard && key.key == "v")
            || (m.shift && !m.control && !m.alt && !m.platform && key.key == "insert")
        {
            self.paste_clipboard(cx);
            cx.stop_propagation();
            return;
        }
        if m.shift && matches!(key.key.as_str(), "pageup" | "pagedown") {
            if let Some(tab) = self
                .exec
                .tabs
                .iter_mut()
                .find(|t| Some(t.key) == self.exec.active)
            {
                let page = usize::from(tab.parser.screen().size().0.get() - 1);
                let offset = tab.parser.screen().scrollback();
                tab.parser
                    .screen_mut()
                    .set_scrollback(if key.key == "pageup" {
                        offset + page
                    } else {
                        offset.saturating_sub(page)
                    });
                tab.refresh_view();
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        if cfg!(target_os = "macos") && m.platform && !m.control && !m.alt && key.key == "a" {
            if let Some(tab) = self
                .exec
                .tabs
                .iter_mut()
                .find(|t| Some(t.key) == self.exec.active)
            {
                let screen = tab.parser.screen();
                tab.selection = Some(super::selection::Selection {
                    end: (
                        tab.parser.callbacks().discarded
                            + screen.scrollback_len()
                            + usize::from(screen.size().0.get() - 1),
                        screen.size().1.get(),
                    ),
                    ..super::selection::Selection::at((tab.parser.callbacks().discarded, 0), false)
                });
                tab.refresh_view();
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        let application = self
            .exec
            .active()
            .is_some_and(|t| t.parser.screen().application_cursor());
        let modifier = 1
            + u8::from(m.shift)
            + 2 * u8::from(m.alt)
            + 4 * u8::from(m.control)
            + 8 * u8::from(m.platform);
        let navigation = match key.key.as_str() {
            "up" => Some('A'),
            "down" => Some('B'),
            "right" => Some('C'),
            "left" => Some('D'),
            "home" => Some('H'),
            "end" => Some('F'),
            _ => None,
        };
        let data = if let Some(code) = navigation {
            if m.platform {
                String::new()
            } else if modifier > 1 {
                format!("\x1b[1;{modifier}{code}")
            } else {
                format!("\x1b{}{code}", if application { 'O' } else { '[' })
            }
        } else if m.control && !m.shift && !m.alt && !m.platform && key.key.len() == 1 {
            match key.key.as_bytes()[0] {
                b'a'..=b'z' => String::from((key.key.as_bytes()[0] & 0x1f) as char),
                b' ' | b'@' => "\0".into(),
                b'[' | b'3' => "\x1b".into(),
                b'\\' | b'4' => "\x1c".into(),
                b']' | b'5' => "\x1d".into(),
                b'6' => "\x1e".into(),
                b'7' | b'_' => "\x1f".into(),
                b'8' => "\x7f".into(),
                _ => String::new(),
            }
        } else {
            match key.key.as_str() {
                "enter" => if m.alt { "\x1b\r" } else { "\r" }.into(),
                "backspace" => format!(
                    "{}{}",
                    if m.alt { "\x1b" } else { "" },
                    if m.control { "\x08" } else { "\x7f" }
                ),
                "tab" => if m.shift { "\x1b[Z" } else { "\t" }.into(),
                "escape" => if m.alt { "\x1b\x1b" } else { "\x1b" }.into(),
                "insert" | "delete" | "pageup" | "pagedown" | "f5" | "f6" | "f7" | "f8" | "f9"
                | "f10" | "f11" | "f12" => {
                    let code = match key.key.as_str() {
                        "insert" => 2,
                        "delete" => 3,
                        "pageup" => 5,
                        "pagedown" => 6,
                        "f5" => 15,
                        "f6" => 17,
                        "f7" => 18,
                        "f8" => 19,
                        "f9" => 20,
                        "f10" => 21,
                        "f11" => 23,
                        _ => 24,
                    };
                    if modifier > 1 {
                        format!("\x1b[{code};{modifier}~")
                    } else {
                        format!("\x1b[{code}~")
                    }
                }
                "f1" | "f2" | "f3" | "f4" => {
                    let code = match key.key.as_str() {
                        "f1" => 'P',
                        "f2" => 'Q',
                        "f3" => 'R',
                        _ => 'S',
                    };
                    if modifier > 1 {
                        format!("\x1b[1;{modifier}{code}")
                    } else {
                        format!("\x1bO{code}")
                    }
                }
                _ if m.alt
                    && !m.control
                    && !m.platform
                    && !cfg!(target_os = "macos")
                    && key.key.len() == 1 =>
                {
                    format!("\x1b{}", key.key)
                }
                _ => String::new(),
            }
        };
        if !data.is_empty() {
            self.exec_input(data, cx);
            cx.stop_propagation();
        }
    }
}
