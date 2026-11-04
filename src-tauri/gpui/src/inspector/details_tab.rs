//! Details tab of the resource detail panel, mirroring `DetailsTab.svelte`
//! and `HealthAssessmentBadge.svelte`.

use super::Inspector;
use super::detail::ResourceDetail;
use super::timeline::{incident_signals, timeline};
use crate::model::{exact_timestamp, health_source};
use crate::theme::Tokens;
use crate::ui::{self, Tone};
use gpui_kit::{Context, Div, SharedString, div, prelude::*, px};
use kubecove_lib::models::{HealthAssessmentState, ResourceSummary};

/// `grid grid-cols-[7rem_minmax(0,1fr)] gap-2 py-1.5`; `metadata_card` adds the row borders.
fn row(label: &str, value: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .py(px(6.))
        .child(ui::muted(ui::xs(label.to_owned())).w(px(112.)).flex_none())
        .child(div().flex().flex_1().min_w_0().child(value))
}

fn value(text: impl Into<SharedString>) -> Div {
    ui::semibold(ui::xs(text)).truncate()
}

/// `rounded-md border bg-background/70 p-3` with a muted `mb-2` title.
fn metadata_card(title: &str, rows: Vec<Div>) -> Div {
    div()
        .p(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.7))
        .child(ui::muted(ui::semibold(ui::xs(title.to_owned()))).mb(px(8.)))
        .children({
            // `border-b last:border-b-0`
            let last = rows.len().saturating_sub(1);
            rows.into_iter().enumerate().map(move |(index, row)| {
                row.when(index < last, |row| {
                    row.border_b_1().border_color(Tokens::border())
                })
            })
        })
}

/// Incident summary from `DetailsTab.svelte`; the signal cards are in `timeline`.
fn incident_summary(detail: &ResourceDetail) -> Div {
    let summary = &detail.data.summary;
    div()
        .p(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::primary().opacity(0.3))
        .bg(Tokens::primary().opacity(0.05))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_start()
                .justify_between()
                .gap(px(8.))
                .child(
                    div()
                        .min_w_0()
                        .child(ui::muted(ui::caps("Incident summary", 12.)))
                        .child(
                            ui::semibold(ui::sm(format!("{}/{}", summary.kind, summary.name)))
                                .mt(px(4.)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap(px(6.))
                        .when_some(summary.status.clone(), |row, status| {
                            row.child(ui::status_badge(Tone::Neutral, status))
                        })
                        .when_some(summary.ready.clone(), |row, ready| {
                            row.child(ui::status_badge(Tone::Neutral, format!("Ready {ready}")))
                        })
                        .when(summary.restarts.unwrap_or(0) > 0, |row| {
                            row.child(ui::status_badge(
                                Tone::Warning,
                                format!("Restarts {}", summary.restarts.unwrap_or(0)),
                            ))
                        }),
                ),
        )
        .child(incident_signals(detail))
}

fn health(summary: &ResourceSummary) -> (Tone, &'static str) {
    match summary.health_assessment.state {
        HealthAssessmentState::Healthy => (Tone::Success, "Healthy"),
        HealthAssessmentState::NeedsAttention => (Tone::Warning, "Needs attention"),
        HealthAssessmentState::Degraded => (Tone::Error, "Degraded"),
        HealthAssessmentState::Unknown => (Tone::Neutral, "Unknown"),
        HealthAssessmentState::NotEvaluated => (Tone::Neutral, "Not evaluated"),
    }
}

pub(super) fn details_tab(detail: &ResourceDetail, cx: &mut Context<Inspector>) -> Div {
    let summary = &detail.data.summary;
    let (tone, label) = health(summary);
    let source = health_source(&summary.health_assessment.winning_sources);
    let created = summary.created_at.as_deref().and_then(exact_timestamp);
    let finalizers =
        detail.data.metadata["finalizers"]
            .as_array()
            .map_or("none".to_owned(), |items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            });
    // One wrapping badge row (the `HealthAssessmentBadge` group and its closed
    // `<details>` are flex items), the incident summary, then the metadata section.
    let ready_badge = |text: String| ui::status_badge(Tone::Neutral, text);
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_start()
                .gap(px(8.))
                .child(ui::outline_badge(summary.kind.clone()))
                .child(ui::outline_badge(
                    summary.namespace.clone().unwrap_or("cluster".into()),
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .min_w_0()
                        .child(ui::status_badge(tone, label))
                        .child(
                            ui::muted(ui::text(format!("Source: {source}"), 11., 20.)).truncate(),
                        ),
                )
                .child(
                    div()
                        .mt(px(8.))
                        .p(px(8.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Tokens::border())
                        .bg(Tokens::background().opacity(0.4))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::icon("play", 8., Tokens::foreground()))
                        .child(ui::medium(ui::xs("Health evidence"))),
                )
                .when_some(summary.ready.clone(), |row, ready| {
                    row.child(ready_badge(format!("Ready {ready}")))
                })
                .child(ready_badge(format!(
                    "Restarts {}",
                    summary
                        .restarts
                        .map_or("not reported".to_owned(), |count| count.to_string())
                ))),
        )
        .child(incident_summary(detail))
        .child(timeline(detail, cx))
        .child(
            div()
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::background().opacity(0.3))
                .child(ui::muted(ui::caps("Metadata", 12.)).mb(px(8.)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(metadata_card(
                            "Identity",
                            vec![
                                row("Name", value(summary.name.clone())),
                                row(
                                    "Namespace",
                                    value(summary.namespace.clone().unwrap_or("Cluster".into())),
                                ),
                            ],
                        ))
                        .child(metadata_card(
                            "Lifecycle",
                            vec![
                                row("Created", value(created.unwrap_or("unknown".into()))),
                                row("Age", value(summary.age.clone())),
                                row("Deletion", ui::status_badge(Tone::Success, "not scheduled")),
                                row("Finalizers", value(finalizers)),
                            ],
                        )),
                ),
        )
}
