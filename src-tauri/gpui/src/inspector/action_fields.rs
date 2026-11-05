//! Cached fields and draft subscriptions for ResourceActionsTab.svelte editors.
use super::{
    Inspector,
    action_drafts::{self, Field},
};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext, Context, Entity, Subscription, Window};

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum GroupKind {
    Image(String),
    Environment(String),
    Quantity(String, bool),
    Metadata(bool),
}
impl GroupKind {
    pub(super) fn field(&self, key: &str) -> Field {
        match self {
            Self::Image(name) => Field::Image(name.clone()),
            Self::Environment(name) => Field::Environment(name.clone(), key.into()),
            Self::Quantity(name, limits) => Field::Quantity(name.clone(), *limits, key.into()),
            Self::Metadata(annotations) => Field::Metadata(*annotations, key.into()),
        }
    }
    pub(super) fn labels(&self, key: &str) -> (String, String) {
        match self {
            Self::Image(_) => ("".into(), "Container image".into()),
            Self::Environment(name) => (
                format!("Environment name {name} {key}"),
                format!("Environment value {name} {key}"),
            ),
            Self::Quantity(name, limits) => {
                let field = if *limits { "Limit" } else { "Request" };
                (
                    format!("{field} {name} {key}"),
                    format!("{field} value {name} {key}"),
                )
            }
            Self::Metadata(annotations) => {
                let field = if *annotations {
                    "Annotations"
                } else {
                    "Labels"
                };
                (format!("{field} key {key}"), format!("{field} value {key}"))
            }
        }
    }
    pub(super) fn new_labels(&self) -> (String, String) {
        match self {
            Self::Environment(name) => (
                format!("New environment name {name}"),
                format!("New environment value {name}"),
            ),
            Self::Quantity(name, limits) => {
                let field = if *limits { "limit" } else { "request" };
                (
                    format!("New {field} name {name}"),
                    format!("New {field} value {name}"),
                )
            }
            Self::Metadata(annotations) => {
                let field = if *annotations {
                    "annotations"
                } else {
                    "labels"
                };
                (format!("New {field} key"), format!("New {field} value"))
            }
            Self::Image(_) => (String::new(), String::new()),
        }
    }
}
pub(super) struct Entry {
    pub key: String,
    pub input: Entity<InputState>,
    pub key_input: Entity<InputState>,
    pub removed: bool,
    pub key_label: gpui_kit::SharedString,
    pub value_label: gpui_kit::SharedString,
    pub value_id: gpui_kit::SharedString,
    pub controls_label: gpui_kit::SharedString,
    pub remove_id: gpui_kit::SharedString,
    _subscription: Subscription,
    _rename: Option<Subscription>,
}
fn input(value: String, window: &mut Window, cx: &mut Context<Inspector>) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).default_value(value))
}
pub(super) fn entry(
    kind: &GroupKind,
    key: String,
    value: String,
    removed: bool,
    placeholder: String,
    window: &mut Window,
    cx: &mut Context<Inspector>,
) -> Entry {
    let (key_label, value_label) = kind.labels(&key);
    let value_id = if let GroupKind::Image(name) = kind {
        format!("image:{name}")
    } else {
        value_label.clone()
    };
    let controls_label = format!("{value_label} controls");
    let remove_id = format!("{value_label} remove");
    let key_input = input(key.clone(), window, cx);
    let key_for_value = key_input.clone();
    let kind_for_value = kind.clone();
    let value_input = cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value)
            .placeholder(placeholder)
    });
    let subscription = cx.subscribe_in(&value_input, window, move |this, input, event, _, cx| {
        if matches!(event, InputEvent::Change) {
            let value = input.read(cx).value().to_string();
            let field = kind_for_value.field(&key_for_value.read(cx).value());
            if let Some(a) = this.actions_state() {
                action_drafts::set(&mut a.edits, &field, value);
                for g in &mut a.editor.groups {
                    if let Some(e) = g.entries.iter_mut().find(|e| e.input == *input) {
                        e.removed = false;
                        break;
                    }
                }
            }
            this.refresh_action_labels(cx);
        }
    });
    let rename = if let GroupKind::Environment(name) = kind {
        let name = name.clone();
        Some(
            cx.subscribe_in(&key_input, window, move |this, input, event, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let key = input.read(cx).value().to_string();
                let merges_baseline = this
                    .actions_state()
                    .and_then(|a| a.state.as_ref())
                    .and_then(|base| base.containers.iter().find(|c| c.name == name))
                    .is_some_and(|c| c.env.iter().any(|v| v.name == key));
                if let Some(a) = this.actions_state() {
                    for g in &mut a.editor.groups {
                        if let Some(e) = g.entries.iter_mut().find(|e| e.key_input == *input) {
                            let env = &mut action_drafts::container(&mut a.edits, &name).env;
                            if let Some(change) = env.iter_mut().find(|v| v.name == e.key) {
                                change.name = key.clone();
                            }
                            let (key_label, value_label) = g.kind.labels(&key);
                            e.key_label = key_label.into();
                            e.value_id = value_label.clone().into();
                            e.controls_label = format!("{value_label} controls").into();
                            e.remove_id = format!("{value_label} remove").into();
                            e.value_label = value_label.into();
                            e.key = key;
                            break;
                        }
                    }
                }
                if merges_baseline {
                    this.prepare_action_editor(crate::actions::RowId::Environment, window, cx);
                }
                this.refresh_action_labels(cx);
            }),
        )
    } else {
        None
    };
    Entry {
        key,
        input: value_input,
        key_input,
        removed,
        key_label: key_label.into(),
        value_label: value_label.into(),
        value_id: value_id.into(),
        controls_label: controls_label.into(),
        remove_id: remove_id.into(),
        _subscription: subscription,
        _rename: rename,
    }
}

pub(super) struct NewEntry {
    pub name: Entity<InputState>,
    pub value: Entity<InputState>,
    _subscription: Subscription,
}
impl NewEntry {
    pub fn new(kind: &GroupKind, window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let (name_placeholder, value_placeholder) = match kind {
            GroupKind::Environment(_) => ("Variable name", "Value"),
            GroupKind::Quantity(_, _) => ("Resource name", "Quantity"),
            _ => ("Key", "Value"),
        };
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(name_placeholder));
        let value = cx.new(|cx| InputState::new(window, cx).placeholder(value_placeholder));
        let subscription =
            cx.subscribe_in(&name, window, |_, _, _: &InputEvent, _, cx| cx.notify());
        Self {
            name,
            value,
            _subscription: subscription,
        }
    }
}
