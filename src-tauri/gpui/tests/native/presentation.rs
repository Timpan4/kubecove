// Failure modes: renders count while diagnostics is off; the view that
// rendered gets no counter. The web logs no render counters, so the shared
// scenarios cannot check this report field.
//
// Interaction recorder paints (#662). The parity driver draws a frame before
// every input, so only raw events here can show these failure modes:
// - an input that draws nothing loses its input-to-paint time when the next
//   input begins before any frame;
// - a measured input requests no frame, so its time waits for an unrelated
//   later redraw and is charged with that frame.
use super::{Controlled, open};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, InputEvent, KeyDownEvent, Keystroke, TestAppContext};
use std::sync::Arc;

fn click(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, ids: &[&'static str]) {
    for id in ids {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(*id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
}

fn counter(
    cx: &mut TestAppContext,
    handle: gpui_kit::AnyWindowHandle,
    name: &str,
) -> Option<String> {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window
            .try_find(format!("render-counter:{name}"))
            .and_then(|row| row.label().map(str::to_owned))
    })
    .unwrap()
}

#[gpui_kit::test]
fn render_counters_count_only_while_diagnostics_is_on(cx: &mut TestAppContext) {
    // `open` has rendered Resources with diagnostics off.
    let handle = open(cx, Arc::new(Controlled::default()));
    click(
        cx,
        handle,
        &[
            "open-settings",
            "settings-section:Diagnostics",
            "setting:Enable diagnostics",
        ],
    );
    assert_eq!(counter(cx, handle, "resources.render"), None);
    assert!(counter(cx, handle, "settings.render").is_some());
    click(
        cx,
        handle,
        &[
            "settings-back",
            "open-resources",
            "open-settings",
            "settings-section:Diagnostics",
        ],
    );
    let resources = counter(cx, handle, "resources.render").expect("no Resources render counter");
    assert!(
        resources
            .split_whitespace()
            .any(|word| word.parse::<u64>().is_ok_and(|count| count > 0)),
        "no render count in {resources}"
    );
}

/// The `app.render` count, read from a forced frame that it includes.
fn app_renders(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) -> u64 {
    let label = counter(cx, handle, "app.render").expect("no app.render counter");
    label
        .split_whitespace()
        .find_map(|word| word.parse::<u64>().ok())
        .unwrap_or_else(|| panic!("no render count in {label}"))
}

/// A key with no binding, with no frame forced around it. Mouse presses
/// redraw the Inspector on their own, so they cannot show an input that
/// changes nothing.
fn unbound_key(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_event(
            KeyDownEvent {
                keystroke: Keystroke::parse("f19").unwrap(),
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn each_recorded_input_draws_one_frame_and_keeps_its_paint_time(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    click(
        cx,
        handle,
        &[
            "open-settings",
            "settings-section:Diagnostics",
            "setting:Enable diagnostics",
            "setting:Record interactions",
            "interactions-clear",
        ],
    );
    // The first key after a click switches the input modality, which redraws.
    unbound_key(cx, handle);
    let first = app_renders(cx, handle);
    let idle = app_renders(cx, handle) - first;
    let before = first + idle;
    unbound_key(cx, handle);
    assert_eq!(
        app_renders(cx, handle) - before,
        idle + 1,
        "an input that changes nothing draws one frame, beside the {idle} forced by the read"
    );
    // Each key begins before any forced frame.
    unbound_key(cx, handle);
    unbound_key(cx, handle);
    unbound_key(cx, handle);
    click(cx, handle, &["interactions-copy"]);
    let copied = cx
        .update(|cx| cx.read_from_clipboard().and_then(|item| item.text()))
        .expect("nothing copied");
    let keys: Vec<serde_json::Value> = copied
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|entry| entry["kind"] == "key")
        .collect();
    assert_eq!(keys.len(), 5, "five key entries: {copied}");
    for entry in &keys {
        assert_eq!(
            entry["inputToPaint"][0]["type"], "keydown",
            "entry without its paint time: {entry}"
        );
    }
}


// Failure mode: a recorded target carries element id text, which ids often
// build from cluster data (#662). Targets name the `test_support()` call site
// instead, so the copy holds no id text at all, here the static
// `settings-section:General` id that an id filter would have kept.
#[gpui_kit::test]
fn recorded_targets_name_the_call_site_not_the_element_id(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    click(
        cx,
        handle,
        &[
            "open-settings",
            "settings-section:Diagnostics",
            "setting:Record interactions",
            "settings-section:General",
            "settings-section:Diagnostics",
            "setting:Record interactions",
            "interactions-copy",
        ],
    );
    let copied = cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .unwrap_or_default();
    assert!(
        copied.contains("[src=src/inspector/settings_page.rs:"),
        "no call-site target in {copied}"
    );
    assert!(!copied.contains("settings-section"), "id text in {copied}");
    assert!(!copied.contains("[id="), "id text in {copied}");
}
