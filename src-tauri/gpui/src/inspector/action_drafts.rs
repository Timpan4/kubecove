//! Draft deltas from resourceActionDrafts.ts. Unedited keys never enter a request.
use crate::actions::RowId;
use kubecove_lib::models::{ResourceActionEdits, ResourceContainerEdit, ResourceEnvEdit};

#[derive(Clone, Debug)]
pub(super) enum Field {
    Image(String),
    Environment(String, String),
    Quantity(String, bool, String),
    Metadata(bool, String),
}

pub(super) fn container<'a>(
    edits: &'a mut ResourceActionEdits,
    name: &str,
) -> &'a mut ResourceContainerEdit {
    if let Some(index) = edits.containers.iter().position(|c| c.name == name) {
        let current = edits.containers.remove(index);
        edits.containers.push(current);
    } else {
        edits.containers.push(ResourceContainerEdit {
            name: name.into(),
            ..Default::default()
        });
    }
    edits.containers.last_mut().expect("container was inserted")
}

pub(super) fn set(edits: &mut ResourceActionEdits, field: &Field, value: String) {
    match field {
        Field::Image(name) => container(edits, name).image = Some(value),
        Field::Environment(name, key) => {
            let env = &mut container(edits, name).env;
            env.retain(|e| &e.name != key);
            env.push(ResourceEnvEdit {
                name: key.clone(),
                value: Some(value),
                remove: None,
            });
        }
        Field::Quantity(name, limits, key) => {
            let c = container(edits, name);
            let map = if *limits {
                &mut c.limits
            } else {
                &mut c.requests
            };
            map.insert(key.clone(), Some(value));
        }
        Field::Metadata(annotations, key) => {
            let map = if *annotations {
                &mut edits.annotations
            } else {
                &mut edits.labels
            };
            map.insert(key.clone(), Some(value));
        }
    }
}

pub(super) fn revert(edits: &mut ResourceActionEdits, row: RowId) {
    match row {
        RowId::Replicas => edits.replicas = None,
        RowId::Labels => edits.labels.clear(),
        RowId::Annotations => edits.annotations.clear(),
        RowId::Image | RowId::Environment | RowId::Resources => {
            for c in &mut edits.containers {
                match row {
                    RowId::Image => c.image = None,
                    RowId::Environment => c.env.clear(),
                    _ => {
                        c.requests.clear();
                        c.limits.clear();
                    }
                }
            }
            edits.containers.retain(|c| {
                c.image.is_some()
                    || !c.env.is_empty()
                    || !c.requests.is_empty()
                    || !c.limits.is_empty()
            });
        }
        _ => {}
    }
}

pub(super) fn count(edits: &ResourceActionEdits, row: RowId) -> usize {
    match row {
        RowId::Replicas => usize::from(edits.replicas.is_some()),
        RowId::Labels => edits.labels.len(),
        RowId::Annotations => edits.annotations.len(),
        RowId::Image => edits
            .containers
            .iter()
            .filter(|c| c.image.is_some())
            .count(),
        RowId::Environment => edits.containers.iter().map(|c| c.env.len()).sum(),
        RowId::Resources => edits
            .containers
            .iter()
            .map(|c| c.requests.len() + c.limits.len())
            .sum(),
        _ => 0,
    }
}

/// pendingRowLabel in resourceActionDrafts.ts.
pub(super) fn pending_label(edits: &ResourceActionEdits, row: RowId) -> Option<String> {
    let count = count(edits, row);
    if count == 0 {
        return None;
    }
    if row == RowId::Image {
        return Some(format!(
            "Pending: {}",
            edits
                .containers
                .iter()
                .filter_map(|c| c.image.as_ref().map(|i| format!("{} {i}", c.name)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let noun = match row {
        RowId::Environment => "environment change",
        RowId::Resources => "resource change",
        RowId::Labels => "label change",
        RowId::Annotations => "annotation change",
        _ => return None,
    };
    Some(format!(
        "Pending: {count} {noun}{}",
        if count == 1 { "" } else { "s" }
    ))
}
