//! Row editors mirror ResourceActionsTab.svelte: mt-3 border-l pl-3,
//! space-y-4 container sections and the key/value/control grid.
use super::super::Inspector;
use super::drafts::{self, Field};
use super::fields::{Entry, GroupKind, entry};
use crate::actions::RowId;
use gpui_kit::component::input::InputState;
use gpui_kit::{Context, Entity, Window};

#[derive(Default)]
pub(super) struct ActionEditor {
    pub groups: Vec<Group>,
    pub viewport: gpui_kit::Pixels,
    pub revisions: Vec<super::rollback_view::Revision>,
}
pub(super) struct Group {
    pub title: gpui_kit::SharedString,
    pub name_label: gpui_kit::SharedString,
    pub value_label: gpui_kit::SharedString,
    pub new_controls_label: gpui_kit::SharedString,
    pub add_id: gpui_kit::SharedString,
    pub entries_id: gpui_kit::SharedString,
    pub kind: GroupKind,
    pub entries: Vec<Entry>,
    pub name: Entity<InputState>,
    pub value: Entity<InputState>,
}

impl Inspector {
    pub(super) fn reset_action_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(a) = self.actions_state() else {
            return;
        };
        let row = a.expanded;
        if row == Some(RowId::Replicas) {
            let value = a
                .replicas
                .or(a.state.as_ref().and_then(|s| s.replicas))
                .unwrap_or(0)
                .to_string();
            a.replicas_input
                .update(cx, |i, cx| i.set_value(value, window, cx));
        } else if let Some(row) = row {
            self.prepare_action_editor(row, window, cx);
        }
    }

    pub(super) fn prepare_action_editor(
        &mut self,
        row: RowId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(actions) = self.actions_state() else {
            return;
        };
        let Some(base) = actions.state.as_ref() else {
            return;
        };
        let mut groups = Vec::new();
        match row {
            RowId::Image | RowId::Environment | RowId::Resources => {
                for c in &base.containers {
                    let draft = actions.edits.containers.iter().find(|d| d.name == c.name);
                    match row {
                        RowId::Image => groups.push((
                            c.name.clone(),
                            GroupKind::Image(c.name.clone()),
                            vec![(
                                String::new(),
                                draft
                                    .and_then(|d| d.image.clone())
                                    .unwrap_or(c.image.clone()),
                                false,
                            )],
                        )),
                        RowId::Environment => {
                            let mut values: Vec<_> = c
                                .env
                                .iter()
                                .map(|e| {
                                    let change =
                                        draft.and_then(|d| d.env.iter().find(|v| v.name == e.name));
                                    (
                                        e.name.clone(),
                                        change
                                            .and_then(|v| v.value.clone())
                                            .or(e.value.clone())
                                            .unwrap_or_default(),
                                        change.and_then(|v| v.remove) == Some(true),
                                    )
                                })
                                .collect();
                            if let Some(d) = draft {
                                values.extend(
                                    d.env
                                        .iter()
                                        .filter(|e| !c.env.iter().any(|v| v.name == e.name))
                                        .map(|e| {
                                            (
                                                e.name.clone(),
                                                e.value.clone().unwrap_or_default(),
                                                false,
                                            )
                                        }),
                                );
                            }
                            groups.push((
                                c.name.clone(),
                                GroupKind::Environment(c.name.clone()),
                                values,
                            ));
                        }
                        _ => {
                            for limits in [false, true] {
                                let base = if limits { &c.limits } else { &c.requests };
                                let changes =
                                    draft.map(|d| if limits { &d.limits } else { &d.requests });
                                let mut values: std::collections::BTreeMap<_, _> = base
                                    .iter()
                                    .map(|(k, v)| (k.clone(), (v.clone(), false)))
                                    .collect();
                                if let Some(changes) = changes {
                                    for (k, v) in changes {
                                        values.insert(
                                            k.clone(),
                                            (
                                                v.clone()
                                                    .or_else(|| base.get(k).cloned())
                                                    .unwrap_or_default(),
                                                v.is_none(),
                                            ),
                                        );
                                    }
                                }
                                groups.push((
                                    format!(
                                        "{} / {}",
                                        c.name,
                                        if limits { "Limits" } else { "Requests" }
                                    ),
                                    GroupKind::Quantity(c.name.clone(), limits),
                                    values.into_iter().map(|(k, (v, r))| (k, v, r)).collect(),
                                ));
                            }
                        }
                    }
                }
            }
            RowId::Labels | RowId::Annotations => {
                let annotations = row == RowId::Annotations;
                let (base, changes) = if annotations {
                    (&base.annotations, &actions.edits.annotations)
                } else {
                    (&base.labels, &actions.edits.labels)
                };
                let mut values = base.clone();
                for (k, v) in changes {
                    if let Some(v) = v {
                        values.insert(k.clone(), v.clone());
                    } else {
                        values.remove(k);
                    }
                }
                groups.push((
                    String::new(),
                    GroupKind::Metadata(annotations),
                    values.into_iter().map(|(k, v)| (k, v, false)).collect(),
                ));
            }
            _ => {}
        }
        let mut editor = ActionEditor {
            viewport: window.viewport_size().height,
            ..Default::default()
        };
        for (title, kind, values) in groups {
            let entries = values
                .into_iter()
                .map(|(k, v, r)| {
                    let placeholder = if let GroupKind::Environment(name) = &kind {
                        if base
                            .containers
                            .iter()
                            .find(|c| &c.name == name)
                            .is_some_and(|c| c.env.iter().any(|e| e.name == k && e.referenced))
                        {
                            "Value from Secret or ConfigMap"
                        } else {
                            "Value"
                        }
                    } else {
                        "Value"
                    };
                    entry(&kind, k, v, r, placeholder.into(), window, cx)
                })
                .collect();
            let pair = actions
                .new_entries
                .entry(kind.clone())
                .or_insert_with(|| super::fields::NewEntry::new(&kind, window, cx));
            let name = pair.name.clone();
            let value = pair.value.clone();
            let gi = editor.groups.len();
            let (name_label, value_label) = kind.new_labels();
            let title = if let GroupKind::Quantity(name, _) = &kind {
                name.clone()
            } else {
                title
            };
            editor.groups.push(Group {
                title: title.into(),
                new_controls_label: format!("{name_label} controls").into(),
                name_label: name_label.into(),
                value_label: value_label.into(),
                add_id: format!("action-add:{gi}").into(),
                entries_id: format!("action-entries:{gi}").into(),
                kind,
                entries,
                name,
                value,
            });
        }
        if row == RowId::Rollback {
            editor.revisions = base
                .revisions
                .iter()
                .map(super::rollback_view::Revision::new)
                .collect();
        }
        actions.editor = editor;
    }

    pub(super) fn remove_action_entry(
        &mut self,
        group: usize,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(a) = self.actions_state() else {
            return;
        };
        let Some(base) = a.state.as_ref() else {
            return;
        };
        let g = &mut a.editor.groups[group];
        let e = &mut g.entries[index];
        let field = g.kind.field(&e.key);
        match &field {
            Field::Environment(name, key) => {
                let in_base = base
                    .containers
                    .iter()
                    .find(|c| &c.name == name)
                    .is_some_and(|c| c.env.iter().any(|v| &v.name == key));
                let env = &mut drafts::container(&mut a.edits, name).env;
                env.retain(|v| &v.name != key);
                if !e.removed && in_base {
                    env.push(kubecove_lib::models::ResourceEnvEdit {
                        name: key.clone(),
                        value: None,
                        remove: Some(true),
                    });
                }
                if !in_base {
                    g.entries.remove(index);
                } else {
                    e.removed = !e.removed;
                }
            }
            Field::Quantity(name, limits, key) => {
                let in_base = base
                    .containers
                    .iter()
                    .find(|c| &c.name == name)
                    .is_some_and(|c| {
                        (if *limits { &c.limits } else { &c.requests }).contains_key(key)
                    });
                let c = drafts::container(&mut a.edits, name);
                let map = if *limits {
                    &mut c.limits
                } else {
                    &mut c.requests
                };
                if e.removed || !in_base {
                    map.remove(key);
                } else {
                    map.insert(key.clone(), None);
                }
                if !in_base {
                    g.entries.remove(index);
                } else {
                    e.removed = !e.removed;
                }
            }
            Field::Metadata(annotations, key) => {
                let in_base = (if *annotations {
                    &base.annotations
                } else {
                    &base.labels
                })
                .contains_key(key);
                let map = if *annotations {
                    &mut a.edits.annotations
                } else {
                    &mut a.edits.labels
                };
                if in_base {
                    map.insert(key.clone(), None);
                } else {
                    map.remove(key);
                }
                g.entries.remove(index);
            }
            Field::Image(_) => {}
        }
        let row = a.expanded;
        if let Some(row) = row {
            self.prepare_action_editor(row, window, cx);
        }
        self.refresh_action_labels(cx);
    }

    pub(super) fn add_action_entry(
        &mut self,
        group: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(a) = self.actions_state() else {
            return;
        };
        let g = &a.editor.groups[group];
        let key = g.name.read(cx).value().trim().to_owned();
        if key.is_empty() {
            return;
        }
        let kind = g.kind.clone();
        if let GroupKind::Environment(name) = &kind
            && a.state.as_ref().is_some_and(|b| {
                b.containers
                    .iter()
                    .find(|c| &c.name == name)
                    .is_some_and(|c| c.env.iter().any(|v| v.name == key))
            })
        {
            return;
        }
        let value = g.value.read(cx).value().to_string();
        drafts::set(&mut a.edits, &kind.field(&key), value.clone());
        let e = entry(&kind, key.clone(), value, false, "Value".into(), window, cx);
        let g = &mut a.editor.groups[group];
        if let Some(existing) = g.entries.iter_mut().find(|entry| entry.key == key) {
            *existing = e;
        } else {
            g.entries.push(e);
        }
        g.name.update(cx, |i, cx| i.set_value("", window, cx));
        g.value.update(cx, |i, cx| i.set_value("", window, cx));
        self.refresh_action_labels(cx);
    }
}
