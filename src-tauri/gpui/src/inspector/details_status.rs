//! Status sections of the generic Details tab, mirroring `DetailsTab.svelte`:
//! the health evidence disclosure from `HealthAssessmentBadge.svelte`, the
//! Conditions table, identity and lifecycle rows, and the raw status details.

use super::Inspector;
use super::argo_tab::raw;
use super::detail::{Detail, ResourceDetail};
use super::details_tab::{row, value};
use crate::incident::{self, ConditionRow};
use crate::model::{exact_timestamp, health_source};
use crate::preferences::TimestampTimezone;
use crate::theme::{MONO, Tokens};
use crate::ui::{self, Tone};
use gpui_kit::{AnyElement, Context, Div, ElementId, TestSupportExt, div, prelude::*, px};
use kubecove_lib::models::{HealthAssessmentEvidence, ResourceSummary};
use serde_json::Value;

impl Inspector {
    /// The open detail, when the tab is showing one.
    fn details_status_detail(&mut self) -> Option<&mut ResourceDetail> {
        match &mut self.detail {
            Detail::Ready(detail) => Some(&mut **detail),
            _ => None,
        }
    }
}

/// `<details>` disclosure from `HealthAssessmentBadge.svelte`. Its open state
/// lives on the detail, so it survives re-renders of the tab.
pub(super) fn health_evidence(
    detail: &ResourceDetail,
    cx: &mut Context<Inspector>,
) -> impl IntoElement {
    let open = detail.evidence_open;
    let evidence = &detail.data.summary.health_assessment.evidence;
    div()
        .id("details-health-evidence")
        .test_support()
        .aria_label("Health evidence")
        .mt(px(8.))
        .p(px(8.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.4))
        .cursor_pointer()
        .on_click(cx.listener(|this, _, _, cx| {
            if let Some(detail) = this.details_status_detail() {
                detail.evidence_open = !detail.evidence_open;
                cx.notify();
            }
        }))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(ui::icon(
                    if open {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    12.,
                    Tokens::foreground(),
                ))
                .child(ui::medium(ui::xs("Health evidence"))),
        )
        .when(open, |toggle| {
            toggle.child(
                div().mt(px(8.)).flex().flex_col().gap(px(6.)).children(
                    evidence
                        .iter()
                        .enumerate()
                        .map(|(index, item)| evidence_line(index, item)),
                ),
            )
        })
}

/// One `<li>`: the source label, then the raw value, reason and historical mark.
fn evidence_line(index: usize, item: &HealthAssessmentEvidence) -> AnyElement {
    let source = health_source(std::slice::from_ref(&item.source));
    let historical = if item.current { "" } else { " (historical)" };
    let text = format!(
        "{source}: {} \u{2014} {}{historical}",
        raw(&item.raw),
        item.reason
    );
    labelled(
        format!("details-evidence:{index}"),
        text,
        div()
            .flex()
            .flex_wrap()
            .child(ui::medium(ui::xs(format!("{source}:"))))
            .child(ui::muted(ui::xs(format!(
                " {} \u{2014} {}{historical}",
                raw(&item.raw),
                item.reason
            )))),
    )
}

/// Conditions table, shown only when the status lists conditions.
pub(super) fn conditions(detail: &ResourceDetail, timezone: TimestampTimezone) -> Option<Div> {
    let conditions = incident::condition_rows(detail.data.status.as_ref());
    if conditions.is_empty() {
        return None;
    }
    let summary = &detail.data.summary;
    let rows = conditions
        .iter()
        .enumerate()
        .map(|(index, condition)| {
            vec![
                cell(ui::xs(condition.kind.clone())),
                cell(ui::status_badge(
                    condition_tone(condition, summary),
                    condition.status.clone(),
                )),
                cell(reason_and_message(index, condition)),
                cell(transition(condition.last_transition.as_deref(), timezone)),
            ]
        })
        .collect();
    Some(section(
        "Conditions",
        table(&["Condition", "Status", "Reason", "Last transition"], rows),
    ))
}

/// `conditionStatusTone` in `resource-status.ts`.
fn condition_tone(condition: &ConditionRow, summary: &ResourceSummary) -> Tone {
    match condition.status.as_str() {
        "True" if failure_condition(condition) => Tone::Warning,
        "True" => Tone::Success,
        "False" if incident::succeeded_pod(summary) && expected_completed_condition(condition) => {
            Tone::Neutral
        }
        "False" => Tone::Error,
        _ => Tone::Warning,
    }
}

fn failure_condition(condition: &ConditionRow) -> bool {
    let kind = condition.kind.trim().to_lowercase();
    let reason = condition
        .reason
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    kind == "failed"
        || kind == "failuretarget"
        || reason.contains("failed")
        || reason.contains("failure")
        || reason == "backofflimitexceeded"
        || reason == "deadlineexceeded"
}

fn expected_completed_condition(condition: &ConditionRow) -> bool {
    let kind = condition.kind.trim().to_lowercase();
    let reason = condition
        .reason
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    reason == "podcompleted"
        || kind == "podreadytostartcontainers"
        || kind == "ready"
        || kind == "containersready"
}

fn reason_and_message(index: usize, condition: &ConditionRow) -> Div {
    let reason = condition.reason.clone().unwrap_or_else(|| "-".to_owned());
    div()
        .min_w_0()
        .child(labelled(
            format!("details-condition-reason:{index}"),
            reason.clone(),
            ui::xs(reason),
        ))
        .when_some(condition.message.clone(), |reason_cell, message| {
            reason_cell.child(div().mt(px(4.)).child(labelled(
                format!("details-condition-message:{index}"),
                message.clone(),
                ui::muted(ui::xs(message)),
            )))
        })
}

fn transition(value: Option<&str>, timezone: TimestampTimezone) -> Div {
    match value {
        Some(value) => ui::xs(exact_timestamp(value, timezone).unwrap_or_else(|| value.to_owned()))
            .whitespace_nowrap(),
        None => ui::xs("-"),
    }
}

/// Identity rows from `buildCuratedMetadata`. Missing optional fields are omitted.
pub(super) fn identity_rows(detail: &ResourceDetail) -> Vec<Div> {
    let metadata = &detail.data.metadata;
    let summary = &detail.data.summary;
    let mut rows = vec![
        field_row(
            "Name",
            text(metadata, "name").unwrap_or_else(|| summary.name.clone()),
        ),
        field_row(
            "Namespace",
            text(metadata, "namespace")
                .or_else(|| summary.namespace.clone())
                .unwrap_or_else(|| "cluster-scoped".to_owned()),
        ),
    ];
    if let Some(uid) = text(metadata, "uid") {
        rows.push(field_row("UID", uid));
    }
    if let Some(version) = text(metadata, "resourceVersion") {
        rows.push(field_row("Resource Version", version));
    }
    rows
}

/// Lifecycle rows from `buildCuratedMetadata`. Deletion reads the object's
/// `deletionTimestamp` and shows "not scheduled" only when it is absent.
pub(super) fn lifecycle_rows(detail: &ResourceDetail, timezone: TimestampTimezone) -> Vec<Div> {
    let metadata = &detail.data.metadata;
    let summary = &detail.data.summary;
    let mut rows = Vec::new();
    if let Some(created) =
        text(metadata, "creationTimestamp").or_else(|| summary.created_at.clone())
    {
        rows.push(timestamp_row("Created", &created, timezone));
    }
    if !summary.age.is_empty() {
        rows.push(field_row("Age", summary.age.clone()));
    }
    if let Some(generation) = primitive(&metadata["generation"]) {
        rows.push(field_row("Generation", generation));
    }
    rows.push(match text(metadata, "deletionTimestamp") {
        Some(deleted) => timestamp_row("Deletion", &deleted, timezone),
        None => row("Deletion", ui::status_badge(Tone::Success, "not scheduled")),
    });
    rows.push(field_row("Finalizers", finalizers(&metadata["finalizers"])));
    rows
}

pub(super) fn naming_rows(detail: &ResourceDetail) -> Vec<Div> {
    let mut rows = Vec::new();
    if let Some(name) = text(&detail.data.metadata, "generateName") {
        rows.push(field_row("Generate name", name));
    }
    rows.push(field_row("Kind", detail.data.summary.kind.clone()));
    if let Some(api) = &detail.data.summary.api_version {
        rows.push(field_row("API Version", api.clone()));
    }
    rows
}

fn timestamp_row(label: &str, value: &str, timezone: TimestampTimezone) -> Div {
    field_row(
        label,
        exact_timestamp(value, timezone).unwrap_or_else(|| value.to_owned()),
    )
}

fn field_row(label: &str, text: String) -> Div {
    let field = labelled(format!("details-field:{label}"), text.clone(), value(text));
    row(label, field)
}

fn text(metadata: &Value, key: &str) -> Option<String> {
    metadata[key]
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// `primitiveValue` in `metadata-details.ts`: strings, numbers and booleans.
fn primitive(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
        _ => None,
    }
}

/// `finalizersLabel` in `metadata-details.ts`.
fn finalizers(value: &Value) -> String {
    let names: Vec<&str> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

/// Raw status key/value table. Nested values use pretty JSON, as `formatRawValue` does.
pub(super) fn status_details(detail: &ResourceDetail) -> Option<Div> {
    let Some(Value::Object(status)) = detail.data.status.as_ref() else {
        return None;
    };
    if status.is_empty() {
        return None;
    }
    let rows = status
        .iter()
        .map(|(key, value)| {
            let text = raw_status_value(value);
            vec![
                cell(labelled(
                    format!("details-status-key:{key}"),
                    key.clone(),
                    ui::xs(key.clone()),
                )),
                cell(labelled(
                    format!("details-status:{key}"),
                    text.clone(),
                    ui::xs(text).font_family(MONO),
                )),
            ]
        })
        .collect();
    Some(section("Status details", table(&["Status", "Value"], rows)))
}

fn raw_status_value(value: &Value) -> String {
    match value {
        Value::Null => "-".to_owned(),
        Value::String(text) => text.clone(),
        Value::Number(_) | Value::Bool(_) => value.to_string(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    }
}

/// Element with a test label equal to its rendered text.
fn labelled(id: String, text: String, content: Div) -> AnyElement {
    div()
        .id(ElementId::Name(id.into()))
        .test_support()
        .aria_label(text)
        .child(content)
        .into_any_element()
}

fn cell(content: impl IntoElement) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .px(px(12.))
        .py(px(8.))
        .child(content)
}

fn section(title: &str, body: Div) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(ui::muted(ui::caps(title, 12.)))
        .child(body)
}

/// Bordered table with a `h-10` header row and borders between body rows.
fn table(headers: &[&str], rows: Vec<Vec<Div>>) -> Div {
    let last = rows.len().saturating_sub(1);
    div()
        .overflow_hidden()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.8))
        .child(
            div()
                .flex()
                .items_center()
                .h(px(40.))
                .border_b_1()
                .border_color(Tokens::border())
                .children(
                    headers
                        .iter()
                        .map(|header| cell(ui::muted(ui::xs(*header)))),
                ),
        )
        .children(rows.into_iter().enumerate().map(move |(index, cells)| {
            div()
                .flex()
                .items_center()
                .when(index < last, |row| {
                    row.border_b_1().border_color(Tokens::border())
                })
                .children(cells)
        }))
}
