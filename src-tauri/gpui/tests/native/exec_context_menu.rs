//! The exec terminal's right-click menu. The web gets this menu from the webview,
//! which the web driver cannot open, so these tests drive GPUI's own menu.
use super::Controlled;
use super::session_controls::pod;
use gpui_kit::test::{ClickOptions, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, ClipboardItem, InputEvent, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext,
    point, px,
};
use std::sync::Arc;

/// The first cell of row 2, below the "Starting" banner and its blank line, inside the
/// terminal's px-2.5 pt-2 padding and 14 px rows (as the parity terminal driver measures).
const HELLO: (f32, f32) = (12., 8. + 2.5 * 14.);

/// A terminal whose row 2 reads "hello world", with its first paint and output done.
fn terminal_with_output(cx: &mut TestAppContext) -> AnyWindowHandle {
    let backend = Arc::new(Controlled::default());
    *backend.sessions.exec_output.lock().unwrap() = Some("hello world\r\n".into());
    let handle = pod(cx, backend);
    // The first paint sizes the session, and its output arrives after that.
    for _ in 0..2 {
        cx.update_window(handle, |_, w, cx| w.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }
    handle
}

/// A right click on the first cell of "hello", which opens the menu.
fn open_menu(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, w, cx| {
        w.click_with_options(
            "exec-terminal",
            ClickOptions::new()
                .with_offset(point(px(HELLO.0), px(HELLO.1)))
                .with_button(MouseButton::Right),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(
            w.try_find("popup-menu").is_some(),
            "a right click opens the terminal menu"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn right_click_then_copy_copies_the_selection(cx: &mut TestAppContext) {
    let handle = terminal_with_output(cx);
    // Double-clicking the first cell of "hello" selects the word.
    cx.update_window(handle, |_, w, cx| {
        w.click_with_options(
            "exec-terminal",
            ClickOptions::new()
                .with_offset(point(px(HELLO.0), px(HELLO.1)))
                .with_count(2),
            cx,
        );
        w.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    open_menu(cx, handle);
    cx.update_window(handle, |_, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
    })
    .unwrap();

    let copy = cx
        .update_window(handle, |_, w, _| {
            w.within("popup-menu")
                .find_by_label("Copy")
                .bounds()
                .center()
        })
        .unwrap();
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            MouseDownEvent {
                button: MouseButton::Left,
                position: copy,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            MouseUpEvent {
                button: MouseButton::Left,
                position: copy,
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(
            w.try_find("popup-menu").is_none(),
            "choosing Copy closes the menu"
        );
        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("hello".to_owned())
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn terminal_menu_closes_on_escape_and_outside_click(cx: &mut TestAppContext) {
    let handle = terminal_with_output(cx);
    open_menu(cx, handle);
    cx.update_window(handle, |_, w, cx| {
        w.press("escape", cx);
        w.render_frame(cx);
        assert!(w.try_find("popup-menu").is_none(), "Escape closes the menu");
    })
    .unwrap();

    open_menu(cx, handle);
    // Well outside the menu, which opens at the right click's cell.
    cx.update_window(handle, |_, w, cx| {
        w.click_with_options(
            "exec-terminal",
            ClickOptions::new().with_offset(point(px(400.), px(200.))),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(
            w.try_find("popup-menu").is_none(),
            "an outside click closes the menu"
        );
    })
    .unwrap();
}
