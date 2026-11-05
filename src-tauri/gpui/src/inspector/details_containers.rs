//! The Containers table of the Details tab for a Pod, with the bordered table
//! primitives that the Advanced metadata table shares. Mirrors the Containers
//! section of `DetailsTab.svelte` and `getContainerStatusRows` in `helpers.ts`.

use super::detail::ResourceDetail;
use crate::model::exact_timestamp;
use crate::preferences::TimestampTimezone;
use crate::theme::Tokens;
use crate::ui::{self, Tone};
use gpui_kit::{Div, TestSupportExt, div, prelude::*, px};
use serde_json::{Map, Value};

/// The Containers table; `None` when the Pod status lists no containers.
pub(super) fn containers_section(
    detail: &ResourceDetail,
    timezone: TimestampTimezone,
) -> Option<Div> {
    let containers = containers(detail.data.status.as_ref(), &detail.data.summary.kind);
    if containers.is_empty() {
        return None;
    }
    let rows = containers
        .iter()
        .enumerate()
        .map(|(index, container)| container_row(index, container, timezone))
        .collect();
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                ui::muted(ui::caps("Containers", 12.))
                    .id("containers-title")
                    .test_support()
                    .aria_label("Containers"),
            )
            .child(table(
                &["Container", "Ready", "Restarts", "State", "Last finished"],
                rows,
            )),
    )
}

/// A bordered table; the body rows and header cells split the width equally.
pub(super) fn table_body(headers: &[&str], rows: Vec<Div>) -> Div {
    div()
        .flex()
        .flex_col()
        .child(row_of(
            headers
                .iter()
                .map(|header| cell(ui::semibold(ui::muted(ui::xs(*header)))))
                .collect(),
        ))
        .children(rows)
}

pub(super) fn row_of(cells: Vec<Div>) -> Div {
    div()
        .flex()
        .border_b_1()
        .border_color(Tokens::border())
        .children(cells)
}

pub(super) fn cell(content: impl IntoElement) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .px(px(12.))
        .py(px(8.))
        .child(content)
}

fn table(headers: &[&str], rows: Vec<Div>) -> Div {
    div()
        .overflow_hidden()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.8))
        .child(table_body(headers, rows))
}

#[derive(Default, Clone)]
struct StateFields {
    state: Option<String>,
    reason: Option<String>,
    exit_code: Option<i64>,
    finished_at: Option<String>,
}

struct Container {
    name: String,
    ready: Option<bool>,
    restarts: i64,
    state: StateFields,
    last: StateFields,
}

impl Container {
    /// `isCleanCompletedContainer`.
    fn clean_completed(&self) -> bool {
        self.state.state.as_deref() == Some("terminated")
            && self.state.reason.as_deref() == Some("Completed")
            && self.state.exit_code.unwrap_or(0) == 0
    }

    /// `containerTone`.
    fn tone(&self) -> Tone {
        if self.clean_completed() {
            return Tone::Neutral;
        }
        if self.state.exit_code.is_some_and(|code| code != 0) {
            return Tone::Error;
        }
        if self.state.state.as_deref() == Some("waiting")
            || self.last.exit_code.is_some_and(|code| code != 0)
        {
            return Tone::Warning;
        }
        match (self.ready, self.state.state.as_deref()) {
            (Some(false), _) => Tone::Error,
            (Some(true), _) | (None, Some("running")) => Tone::Success,
            _ => Tone::Neutral,
        }
    }

    /// `containerReadyLabel`.
    fn ready_label(&self) -> &'static str {
        if self.clean_completed() {
            "Completed"
        } else {
            match self.ready {
                None => "Not reported",
                Some(true) => "Yes",
                Some(false) => "No",
            }
        }
    }

    /// `containerStateLabel`: the state, then the current or last reason.
    fn state_label(&self) -> String {
        let state = self.state.state.as_deref().unwrap_or("-");
        let reason = self.state.reason.as_deref().or(self.last.reason.as_deref());
        match reason {
            Some(reason) if state != "-" => format!("{state} - {reason}"),
            Some(reason) => reason.to_owned(),
            None => state.to_owned(),
        }
    }
}

/// `getContainerStatusRows`: init, regular and ephemeral containers of a Pod.
fn containers(status: Option<&Value>, kind: &str) -> Vec<Container> {
    let Some(status) = status.filter(|_| kind == "Pod") else {
        return Vec::new();
    };
    let mut containers = Vec::new();
    for key in [
        "initContainerStatuses",
        "containerStatuses",
        "ephemeralContainerStatuses",
    ] {
        let Some(items) = status.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items.iter().filter_map(Value::as_object) {
            containers.push(Container {
                name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("container")
                    .to_owned(),
                ready: item.get("ready").and_then(Value::as_bool),
                restarts: item
                    .get("restartCount")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                state: state_fields(item.get("state")),
                last: state_fields(item.get("lastState")),
            });
        }
    }
    containers
}

/// `parseStateFields`: the first object entry of a container state.
fn state_fields(state: Option<&Value>) -> StateFields {
    let Some((name, body)) = state
        .and_then(Value::as_object)
        .and_then(|map| map.iter().find(|(_, value)| value.is_object()))
    else {
        return StateFields::default();
    };
    let body: &Map<String, Value> = body.as_object().expect("filtered to objects");
    let text = |key: &str| {
        body.get(key)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    StateFields {
        state: Some(name.clone()),
        reason: text("reason"),
        exit_code: body.get("exitCode").and_then(Value::as_i64),
        finished_at: text("finishedAt"),
    }
}

fn container_row(index: usize, container: &Container, timezone: TimestampTimezone) -> Div {
    let restart_tone = if container.restarts > 0 {
        Tone::Warning
    } else {
        Tone::Neutral
    };
    let last_finished = container
        .last
        .finished_at
        .as_deref()
        .and_then(|value| exact_timestamp(value, timezone))
        .unwrap_or_else(|| "-".into());
    let state_label = container.state_label();
    row_of(vec![
        cell(
            ui::xs(container.name.clone())
                .id(format!("container-name:{index}"))
                .test_support()
                .aria_label(container.name.clone()),
        ),
        cell(
            ui::status_badge(container.tone(), container.ready_label())
                .id(format!("container-ready:{index}"))
                .test_support()
                .aria_label(container.ready_label()),
        ),
        cell(ui::status_badge(
            restart_tone,
            container.restarts.to_string(),
        )),
        cell(
            ui::xs(state_label.clone())
                .id(format!("container-state:{index}"))
                .test_support()
                .aria_label(state_label),
        ),
        cell(ui::xs(last_finished)),
    ])
}
