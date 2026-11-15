//! Failure modes (ADR 0035): a reference from an older frame lands on whatever replaced
//! its element, including another row that took its index; a refused or cancelled
//! request still dispatches events; a label click misses the one control it names.
use super::{Controlled, open};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext, TestAppContext};
use kubecove_gpui::automation::ui::{self, Button, Operation, References, Reply, Target};
use serde_json::Value;
use std::sync::Arc;

fn perform_unless(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    references: &mut References,
    cancelled: bool,
    operation: Operation,
) -> Result<Reply, String> {
    let result = cx
        .update_window(handle, |_, window, cx| {
            ui::perform(operation, references, &|| cancelled, window, cx)
        })
        .unwrap();
    cx.run_until_parked();
    result
}

fn perform(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    references: &mut References,
    operation: Operation,
) -> Result<Reply, String> {
    perform_unless(cx, handle, references, false, operation)
}

fn snapshot(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    references: &mut References,
) -> Value {
    let operation = Operation::Snapshot {
        include_hidden: false,
    };
    match perform(cx, handle, references, operation) {
        Ok(Reply::Json(value)) => value,
        Ok(Reply::Png(_)) => panic!("snapshot returned an image"),
        Err(error) => panic!("snapshot failed: {error}"),
    }
}

/// The reference of the visible element whose path ends with `id`.
fn reference(snapshot: &Value, id: &str) -> Option<String> {
    snapshot["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|element| {
            element["path"]
                .as_str()
                .is_some_and(|path| path.rsplit('/').next() == Some(id))
        })
        .map(|element| element["ref"].as_str().unwrap().to_owned())
}

fn click(target: Target) -> Operation {
    Operation::Click {
        target,
        button: Button::Left,
        count: 1,
    }
}

#[gpui_kit::test]
fn a_reference_whose_element_left_the_window_fails_without_clicking(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    let mut references = References::default();
    let resources = snapshot(cx, handle, &mut references);
    assert_eq!(reference(&resources, "settings-section:Appearance"), None);

    perform(
        cx,
        handle,
        &mut references,
        click(Target::Label {
            label: "Open settings".into(),
            description: None,
        }),
    )
    .expect("the header button is labeled Open settings");
    let settings = snapshot(cx, handle, &mut references);
    let appearance = reference(&settings, "settings-section:Appearance")
        .expect("Settings lists its Appearance section");
    let back = reference(&settings, "settings-back").expect("Settings has a back control");

    perform(cx, handle, &mut references, click(Target::Reference(back)))
        .expect("the back control is still in the window");
    let error = perform(
        cx,
        handle,
        &mut references,
        click(Target::Reference(appearance)),
    )
    .err()
    .expect("a reference to a closed page must fail");
    assert!(error.contains("take a new snapshot"), "{error}");
    let after = snapshot(cx, handle, &mut references);
    assert_eq!(
        reference(&after, "settings-section:Appearance"),
        None,
        "the refused click reopened Settings"
    );
}

#[gpui_kit::test]
fn refused_requests_leave_the_window_unchanged(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    let mut references = References::default();
    snapshot(cx, handle, &mut references);
    let open_settings = || Target::Label {
        label: "Open settings".into(),
        description: None,
    };
    let refused = [
        click(Target::Reference("e999999".into())),
        click(Target::Label {
            label: "No control has this label".into(),
            description: None,
        }),
        click(Target::Label {
            label: "Open settings".into(),
            description: Some("a description no control has".into()),
        }),
        click(Target::Position(ui::position(-1., 10.))),
        Operation::Click {
            target: open_settings(),
            button: Button::Left,
            count: 0,
        },
        Operation::PressKey { key: "a-b".into() },
        // GPUI's parser takes any word as a key, and the window types it as text.
        Operation::PressKey {
            key: "notakey".into(),
        },
        Operation::PressKey {
            key: "enter enter".into(),
        },
        Operation::PressKey { key: "".into() },
        Operation::PressKey {
            key: "ctrl-".into(),
        },
        Operation::PressKey { key: "a->b".into() },
        // Control characters were dropped or inserted raw while reporting success.
        Operation::TypeText {
            target: None,
            text: "line\nbreak".into(),
        },
        Operation::TypeText {
            target: None,
            text: "tab\there".into(),
        },
        Operation::TypeText {
            target: Some(Target::Label {
                label: "No control has this label".into(),
                description: None,
            }),
            text: "x".into(),
        },
        Operation::Scroll {
            target: Target::Reference("e999999".into()),
            delta: ui::position(0., -40.),
        },
    ];
    for operation in refused {
        assert!(perform(cx, handle, &mut references, operation).is_err());
    }
    let cancelled = perform_unless(cx, handle, &mut references, true, click(open_settings()));
    assert!(cancelled.is_err(), "a cancelled click must fail");
    let after = snapshot(cx, handle, &mut references);
    assert_eq!(
        reference(&after, "settings-section:Appearance"),
        None,
        "a refused or cancelled request reached the window"
    );
}

/// The key check must still pass every key a test needs: named keys, function keys,
/// characters with and without modifiers, and the `-` key.
#[gpui_kit::test]
fn real_keys_and_printable_text_are_sent(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    let mut references = References::default();
    for key in [
        "escape",
        "Tab",
        "shift-tab",
        "down",
        "f5",
        "f35",
        "ctrl-a",
        "A",
        "-",
        "shift",
    ] {
        let operation = Operation::PressKey { key: key.into() };
        perform(cx, handle, &mut references, operation)
            .unwrap_or_else(|error| panic!("{key}: {error}"));
    }
    let typed = Operation::TypeText {
        target: None,
        text: "Ab -_./: é€😀".into(),
    };
    perform(cx, handle, &mut references, typed).expect("printable text types");
}

/// Saved forwards key their Delete buttons by row index. Removing the first moves the
/// second into its path, so a reference that resolved by path alone deleted it too.
#[gpui_kit::test]
fn a_reference_whose_path_now_names_another_row_fails(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        w.click("nav:Port Forwards", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        for remote in ["80", "81"] {
            w.render_frame(cx);
            w.click("save-forward", cx);
            w.render_frame(cx);
            for (id, text) in [
                ("preset-namespace", "payments"),
                ("preset-service", "payments-api"),
                ("preset-remote", remote),
            ] {
                w.click(id, cx);
                w.input(text, cx);
            }
            w.click("preset-save", cx);
        }
        w.render_frame(cx);
        assert!(w.try_find("remove-preset:1").is_some(), "two presets saved");
    })
    .unwrap();
    cx.run_until_parked();

    let mut references = References::default();
    let presets = snapshot(cx, handle, &mut references);
    let first = reference(&presets, "remove-preset:0").expect("the first preset's Delete");
    perform(
        cx,
        handle,
        &mut references,
        click(Target::Reference(first.clone())),
    )
    .expect("the first preset's Delete is in the window");
    let error = perform(cx, handle, &mut references, click(Target::Reference(first)))
        .err()
        .expect("the remaining preset took the deleted one's path");
    assert!(error.contains("take a new snapshot"), "{error}");
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(
            w.try_find("remove-preset:0").is_some(),
            "the outdated reference deleted the remaining preset"
        );
    })
    .unwrap();
}
