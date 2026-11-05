use super::{number, string};
use gpui_kit::test::{ClickOptions, TestWindowExt};
use gpui_kit::{App, ClipboardItem, ScrollDelta, Window, point, px};
use serde_json::Value;

pub fn apply(window: &mut Window, cx: &mut App, step: &Value) -> bool {
    match string(step, "action") {
        "terminalFit" => {}
        "terminalOutput" => {}
        "terminalSelect" => {
            let bounds = window.find("exec-terminal").bounds();
            let font = window
                .text_system()
                .resolve_font(&gpui_kit::font(kubecove_gpui::theme::MONO));
            let width = window
                .text_system()
                .advance(font, px(12.), 'm')
                .unwrap()
                .width;
            let offset = |row: &str, col: &str| {
                point(
                    px(10.) + width * (number(step, col) as f32 + 0.1),
                    px(8. + (number(step, row) as f32 + 0.5) * 14.),
                )
            };
            let from = offset("row", "col");
            if step["word"] == true {
                window.click_with_options(
                    "exec-terminal",
                    ClickOptions::new().with_offset(from).with_count(2),
                    cx,
                );
            } else {
                window.drag(
                    bounds.origin + from,
                    bounds.origin + offset("endRow", "endCol"),
                    cx,
                );
            }
        }
        "terminalCopy" => {
            // TestAppContext's platform owns an in-memory clipboard.
            cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-c"
                } else {
                    "ctrl-insert"
                },
                cx,
            );
            assert_eq!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .unwrap_or_default(),
                string(step, "text")
            );
        }
        "terminalPaste" => {
            cx.write_to_clipboard(ClipboardItem::new_string(string(step, "text").to_owned()));
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-v"
                } else {
                    "ctrl-shift-v"
                },
                cx,
            );
        }
        "terminalScroll" => window.scroll(
            "exec-terminal",
            ScrollDelta::Pixels(point(px(0.), px(number(step, "rows") as f32 * 14.))),
            cx,
        ),
        _ => return false,
    }
    true
}
