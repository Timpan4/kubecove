use super::{number, string};
use gpui_kit::test::{ClickOptions, TestWindowExt};
use gpui_kit::{
    App, ClipboardItem, InputEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, ScrollDelta, Window, point, px,
};
use serde_json::Value;

fn cell_width(window: &Window) -> Pixels {
    let font = window
        .text_system()
        .resolve_font(&gpui_kit::font(kubecove_gpui::theme::MONO));
    window
        .text_system()
        .advance(font, px(12.), 'm')
        .unwrap()
        .width
}

/// A cell's offset inside `exec-terminal`, past its px-2.5 pt-2 padding.
fn cell(window: &Window, row: u64, col: u64) -> Point<Pixels> {
    point(
        px(10.) + cell_width(window) * (col as f32 + 0.1),
        px(8. + (row as f32 + 0.5) * 14.),
    )
}

fn send(window: &mut Window, cx: &mut App, event: gpui_kit::PlatformInput) {
    window.dispatch_event(event, cx);
    window.render_frame(cx);
}

fn press(window: &mut Window, cx: &mut App, position: Point<Pixels>, modifiers: Modifiers) {
    send(
        window,
        cx,
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        }
        .to_platform_input(),
    );
    send(
        window,
        cx,
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
    );
}

fn hold(window: &mut Window, cx: &mut App, position: Point<Pixels>, modifiers: Modifiers) {
    send(
        window,
        cx,
        MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            modifiers,
        }
        .to_platform_input(),
    );
}

fn release(window: &mut Window, cx: &mut App, position: Point<Pixels>, modifiers: Modifiers) {
    send(
        window,
        cx,
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
        }
        .to_platform_input(),
    );
}

/// Runs xterm's 50 ms DRAG_SCROLL_INTERVAL for `ms`. Each advance fires one
/// interval, because the drag task re-arms its timer from the advanced clock.
pub fn advance_drag_scroll(cx: &mut gpui_kit::TestAppContext, ms: u64) {
    for _ in 0..ms / 50 {
        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(50));
        cx.run_until_parked();
    }
}

/// Closes the menu a right click opens, as a user does before typing. The web driver's
/// right press opens no menu. GPUI builds the menu after the right click's update, so
/// Escape in that update would reach the terminal and leave the menu holding focus.
pub fn finish(window: &mut Window, cx: &mut App, step: &Value) {
    if step["action"] == "terminalRightClick" {
        assert!(
            window.try_find("popup-menu").is_some(),
            "a right click opens the terminal menu"
        );
        window.press("escape", cx);
    }
}

pub fn apply(window: &mut Window, cx: &mut App, step: &Value) -> bool {
    match string(step, "action") {
        "terminalFit" => {}
        "terminalOutput" => {}
        "terminalSelect" => {
            let origin = window.find("exec-terminal").bounds().origin;
            let from = cell(window, number(step, "row"), number(step, "col"));
            if step["word"] == true || step["line"] == true {
                window.click_with_options(
                    "exec-terminal",
                    ClickOptions::new()
                        .with_offset(from)
                        .with_count(if step["line"] == true { 3 } else { 2 }),
                    cx,
                );
            } else {
                let modifiers = Modifiers {
                    alt: step["alt"] == true,
                    ..Modifiers::default()
                };
                let to = origin + cell(window, number(step, "endRow"), number(step, "endCol"));
                press(window, cx, origin + from, modifiers);
                hold(window, cx, to, modifiers);
                release(window, cx, to, modifiers);
            }
        }
        "terminalDragOutside" => {
            let bounds = window.find("exec-terminal").bounds();
            let from = bounds.origin + cell(window, number(step, "row"), number(step, "col"));
            // Rows fill the terminal below its 8 px top padding, 14 px each.
            let rows = ((f32::from(bounds.size.height) - 8.) / 14.).floor();
            let top = bounds.origin.y + px(8.);
            // 50 px reaches xterm's DRAG_SCROLL_MAX_THRESHOLD.
            let y = if string(step, "edge") == "below" {
                top + px(rows * 14. + 50.)
            } else {
                top - px(50.)
            };
            press(window, cx, from, Modifiers::default());
            hold(window, cx, point(from.x, y), Modifiers::default());
        }
        // parity.rs advances the drag-scroll clock before this release.
        "terminalRelease" => {
            let position = window.mouse_position();
            release(window, cx, position, Modifiers::default());
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
        "terminalPrimaryPaste" => {
            // The test platform's primary selection is in memory, like its clipboard.
            #[cfg(target_os = "linux")]
            {
                assert_eq!(
                    cx.read_from_primary()
                        .and_then(|item| item.text())
                        .unwrap_or_default(),
                    string(step, "text")
                );
                window.click_with_options(
                    "exec-terminal",
                    ClickOptions::new().with_button(MouseButton::Middle),
                    cx,
                );
            }
            #[cfg(not(target_os = "linux"))]
            panic!("Primary selection paste is Linux-only");
        }
        "terminalFocus" => {
            if step["focused"] == true {
                // GPUI reports focus only in an active window; the browser page is one.
                window.activate_window();
                window.click("exec-terminal", cx);
            } else {
                window.blur(cx);
                window.render_frame(cx);
            }
        }
        "terminalRightClick" => {
            // Only the primary button selects, so a right press leaves the selection as it is.
            let position = cell(window, number(step, "row"), number(step, "col"));
            window.click_with_options(
                "exec-terminal",
                ClickOptions::new()
                    .with_offset(position)
                    .with_button(MouseButton::Right),
                cx,
            );
        }
        "expectTerminalCursor" => {
            let origin = window.find("exec-terminal").bounds().origin;
            let cursor = window.find("exec-cursor");
            assert_eq!(cursor.label(), Some(string(step, "cursor")));
            let at = cursor.bounds().origin - origin;
            if let Some(row) = step["row"].as_u64() {
                assert_eq!(((f32::from(at.y) - 8.) / 14.).round() as u64, row);
            }
            if let Some(col) = step["col"].as_u64() {
                let width = f32::from(cell_width(window));
                assert_eq!(((f32::from(at.x) - 10.) / width).round() as u64, col);
            }
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
