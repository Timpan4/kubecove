// Failure modes: renders count while diagnostics is off; the view that
// rendered gets no counter. The web logs no render counters, so the shared
// scenarios cannot check this report field.
use super::{Controlled, open};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext};
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
