//! Window operations behind the automation tools (ADR 0035). They read GPUI Kit's
//! per-frame element observations and send pointer, wheel and key events through the
//! window's own dispatch, as the parity driver does. Every request is checked before
//! its first event, so a refused request changes nothing.

use gpui_kit::test::{ElementSnapshot, TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{
    App, Bounds, ElementId, InputEvent as _, Keystroke, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta, ScrollEvent, Window, point, px,
};
use serde_json::{Map, Value, json};
use std::hash::{DefaultHasher, Hash as _, Hasher as _};

/// What a tool asks of the window.
pub enum Operation {
    Snapshot {
        include_hidden: bool,
    },
    Click {
        target: Target,
        button: Button,
        count: usize,
    },
    TypeText {
        target: Option<Target>,
        text: String,
    },
    PressKey {
        key: String,
    },
    Scroll {
        target: Target,
        delta: Point<Pixels>,
    },
    Screenshot,
}

/// Where an action lands.
pub enum Target {
    /// A reference such as `e12` from the latest snapshot.
    Reference(String),
    /// The one visible element with this exact accessible label, and description when given.
    Label {
        label: String,
        description: Option<String>,
    },
    /// A window position in pixels.
    Position(Point<Pixels>),
}

#[derive(Clone, Copy)]
pub enum Button {
    Left,
    Right,
    Middle,
}

pub enum Reply {
    Json(Value),
    Png(Vec<u8>),
}

/// The element paths behind the latest snapshot's references; `e<n>` is entry `n`.
/// `identity` fingerprints every element the snapshot saw, because KubeCove's element
/// ids can give a path to another item, such as a row index after a row is removed.
#[derive(Default)]
pub struct References {
    paths: Vec<Vec<ElementId>>,
    identity: u64,
}

/// Performs one operation. `cancelled` reports that the caller stopped waiting; it is
/// checked before each event, so a cancelled request sends nothing further.
pub fn perform(
    operation: Operation,
    references: &mut References,
    cancelled: &dyn Fn() -> bool,
    window: &mut Window,
    cx: &mut App,
) -> Result<Reply, String> {
    window.render_frame(cx);
    match operation {
        Operation::Snapshot { include_hidden } => {
            Ok(Reply::Json(snapshot(include_hidden, references, window)))
        }
        Operation::Click {
            target,
            button,
            count,
        } => {
            if count == 0 {
                return Err("count must be at least 1".into());
            }
            let position = resolve(&target, references, window)?;
            click(position, button, count, cancelled, window, cx)?;
            Ok(Reply::Json(action(position)))
        }
        Operation::TypeText { target, text } => {
            let keys = typed_keys(&text, cx)?;
            let position = target
                .map(|target| resolve(&target, references, window))
                .transpose()?;
            if let Some(position) = position {
                click(position, Button::Left, 1, cancelled, window, cx)?;
            }
            for key in keys {
                proceed(cancelled)?;
                window.dispatch_keystroke(key, cx);
                window.render_frame(cx);
            }
            Ok(Reply::Json(json!({ "typed": text.chars().count() })))
        }
        Operation::PressKey { key } => {
            Keystroke::parse_with_os(&key, cx.platform_info().os)
                .map_err(|error| format!("invalid key {key:?}: {error}"))?;
            proceed(cancelled)?;
            window.press(&key, cx);
            Ok(Reply::Json(json!({ "pressed": key })))
        }
        Operation::Scroll { target, delta } => {
            let position = resolve(&target, references, window)?;
            proceed(cancelled)?;
            let modifiers = window.modifiers();
            move_pointer(position, window, cx);
            window.dispatch_event(
                ScrollEvent {
                    position,
                    delta: ScrollDelta::Pixels(delta),
                    modifiers,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            Ok(Reply::Json(action(position)))
        }
        Operation::Screenshot => window
            .render_to_image()
            .map_err(|error| {
                format!(
                    "this platform's GPUI window cannot capture its pixels ({error}); use snapshot"
                )
            })
            .and_then(|image| {
                let mut png = std::io::Cursor::new(Vec::new());
                image
                    .write_to(&mut png, image::ImageFormat::Png)
                    .map_err(|error| format!("could not encode the screenshot: {error}"))?;
                Ok(Reply::Png(png.into_inner()))
            }),
    }
}

fn proceed(cancelled: &dyn Fn() -> bool) -> Result<(), String> {
    if cancelled() {
        Err("tool call cancelled".into())
    } else {
        Ok(())
    }
}

/// A fingerprint of which elements the window lists: their paths, roles, labels and
/// descriptions. Values and bounds are left out, so typing or scrolling within the same
/// elements keeps references valid.
fn identity(elements: &[ElementSnapshot]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for element in elements {
        element.path().hash(&mut hasher);
        element
            .role()
            .map(|role| format!("{role:?}"))
            .hash(&mut hasher);
        element.label().hash(&mut hasher);
        element.description().hash(&mut hasher);
    }
    hasher.finish()
}

fn snapshot(include_hidden: bool, references: &mut References, window: &Window) -> Value {
    let elements = window.elements();
    references.identity = identity(&elements);
    let elements: Vec<ElementSnapshot> = elements
        .into_iter()
        .filter(|element| include_hidden || element.visible())
        .collect();
    references.paths = elements
        .iter()
        .map(|element| element.path().to_vec())
        .collect();
    let elements: Vec<Value> = elements
        .iter()
        .enumerate()
        .map(|(index, element)| describe(index, element))
        .collect();
    let texts: Vec<Value> = gpui_kit::base::test_support::texts(window)
        .iter()
        .filter(|text| include_hidden || text.visible())
        .map(|text| {
            json!({
                "text": text.text(),
                "bounds": bounds(text.bounds()),
                "path": path(text.path()),
            })
        })
        .collect();
    let size = window.viewport_size();
    json!({
        "window": { "width": f32::from(size.width), "height": f32::from(size.height) },
        "elements": elements,
        "texts": texts,
    })
}

fn describe(index: usize, element: &ElementSnapshot) -> Value {
    let mut fields = Map::new();
    fields.insert("ref".into(), json!(format!("e{index}")));
    fields.insert("path".into(), json!(path(element.path())));
    if let Some(role) = element.role() {
        fields.insert("role".into(), json!(format!("{role:?}")));
    }
    let texts = [
        ("label", element.label()),
        ("description", element.description()),
        ("value", element.value()),
        ("placeholder", element.placeholder()),
    ];
    for (name, value) in texts {
        if let Some(value) = value {
            fields.insert(name.into(), json!(value));
        }
    }
    let states = [
        ("checked", element.checked()),
        ("indeterminate", element.indeterminate()),
        ("selected", element.selected()),
        ("expanded", element.expanded()),
        ("disabled", element.disabled()),
        ("focused", focus(element)),
    ];
    for (name, value) in states {
        if let Some(value) = value {
            fields.insert(name.into(), json!(value));
        }
    }
    if !element.visible() {
        fields.insert("hidden".into(), json!(true));
    }
    fields.insert("bounds".into(), bounds(element.bounds()));
    Value::Object(fields)
}

/// `ElementSnapshot::focused` panics for an element that advertises focus without an
/// observed binding. GPUI Kit 0.9.0 exposes that case only in the Debug form.
fn focus(element: &ElementSnapshot) -> Option<bool> {
    if format!("{element:?}").contains("focused: <binding missed>") {
        None
    } else {
        element.focused()
    }
}

fn path(path: &[ElementId]) -> String {
    path.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("/")
}

fn bounds(bounds: Bounds<Pixels>) -> Value {
    json!({
        "x": f32::from(bounds.origin.x),
        "y": f32::from(bounds.origin.y),
        "width": f32::from(bounds.size.width),
        "height": f32::from(bounds.size.height),
    })
}

fn action(position: Point<Pixels>) -> Value {
    json!({ "at": { "x": f32::from(position.x), "y": f32::from(position.y) } })
}

/// The window position an action targets, or why it cannot land.
fn resolve(
    target: &Target,
    references: &References,
    window: &Window,
) -> Result<Point<Pixels>, String> {
    let element = match target {
        Target::Position(position) => {
            let size = window.viewport_size();
            let inside = position.x >= px(0.)
                && position.y >= px(0.)
                && position.x < size.width
                && position.y < size.height;
            return if inside {
                Ok(*position)
            } else {
                Err(format!(
                    "position ({}, {}) is outside the {}x{} window",
                    f32::from(position.x),
                    f32::from(position.y),
                    f32::from(size.width),
                    f32::from(size.height)
                ))
            };
        }
        Target::Reference(reference) => {
            let path = reference
                .strip_prefix('e')
                .and_then(|index| index.parse::<usize>().ok())
                .and_then(|index| references.paths.get(index))
                .ok_or_else(|| format!("unknown reference {reference:?}; take a snapshot"))?;
            let elements = window.elements();
            if identity(&elements) != references.identity {
                return Err(format!(
                    "the window's elements changed since the snapshot that gave {reference}; \
                     take a new snapshot"
                ));
            }
            elements
                .into_iter()
                .find(|element| element.path() == path.as_slice())
                .ok_or_else(|| format!("{reference} is not in the window; take a new snapshot"))?
        }
        Target::Label { label, description } => {
            let matches: Vec<ElementSnapshot> = window
                .find_all_by_label(label)
                .into_iter()
                .filter(|element| {
                    description
                        .as_deref()
                        .is_none_or(|description| element.description() == Some(description))
                })
                .collect();
            let hidden = matches.iter().filter(|element| !element.visible()).count();
            let mut visible: Vec<ElementSnapshot> = matches
                .into_iter()
                .filter(ElementSnapshot::visible)
                .collect();
            match visible.len() {
                1 => visible.remove(0),
                0 if hidden > 0 => {
                    return Err(format!(
                        "{hidden} element(s) labeled {label:?} are hidden; scroll or open them first"
                    ));
                }
                0 => return Err(format!("no element is labeled {label:?}")),
                count => {
                    return Err(format!(
                        "{count} visible elements are labeled {label:?}; give a description or a reference"
                    ));
                }
            }
        }
    };
    if !element.visible() {
        return Err("the target is hidden; scroll or open it first".into());
    }
    Ok(element.bounds().center())
}

fn typed_keys(text: &str, cx: &App) -> Result<Vec<Keystroke>, String> {
    text.chars()
        .map(|character| {
            let text = character.to_string();
            let mut key = Keystroke::parse_with_os(&text, cx.platform_info().os).map_err(|_| {
                format!("{character:?} cannot be typed; send it with press_key instead")
            })?;
            key.key_char = Some(text);
            Ok(key)
        })
        .collect()
}

fn click(
    position: Point<Pixels>,
    button: Button,
    count: usize,
    cancelled: &dyn Fn() -> bool,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), String> {
    let button = match button {
        Button::Left => MouseButton::Left,
        Button::Right => MouseButton::Right,
        Button::Middle => MouseButton::Middle,
    };
    let modifiers = window.modifiers();
    proceed(cancelled)?;
    move_pointer(position, window, cx);
    for click_count in 1..=count {
        proceed(cancelled)?;
        window.dispatch_event(
            MouseDownEvent {
                button,
                position,
                modifiers,
                click_count,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
        window.dispatch_event(
            MouseUpEvent {
                button,
                position,
                modifiers,
                click_count,
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }
    Ok(())
}

fn move_pointer(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    let modifiers = window.modifiers();
    window.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

/// A window position from tool arguments.
pub fn position(x: f32, y: f32) -> Point<Pixels> {
    point(px(x), px(y))
}
