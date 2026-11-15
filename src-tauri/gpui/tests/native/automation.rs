//! Failure modes (ADR 0035): a reference from an older frame lands on whatever replaced
//! its element; a refused request still dispatches events; a label click misses the
//! one control it names.
use super::{Controlled, open};
use gpui_kit::{AnyWindowHandle, AppContext, TestAppContext};
use kubecove_gpui::automation::ui::{self, Button, Operation, References, Reply, Target};
use serde_json::Value;
use std::sync::Arc;

fn perform(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    references: &mut References,
    operation: Operation,
) -> Result<Reply, String> {
    let result = cx
        .update_window(handle, |_, window, cx| {
            ui::perform(operation, references, window, cx)
        })
        .unwrap();
    cx.run_until_parked();
    result
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
    assert!(error.contains("no longer in the window"), "{error}");
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
    let after = snapshot(cx, handle, &mut references);
    assert_eq!(
        reference(&after, "settings-section:Appearance"),
        None,
        "a refused request reached the window"
    );
}
