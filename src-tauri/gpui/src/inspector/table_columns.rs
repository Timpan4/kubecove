//! Resource table columns and cell helpers, mirroring `ResourceBrowser.svelte` and
//! `src/features/resources/helpers.ts`.

use crate::table_sort::Column as SortColumn;
use crate::targets::TestSupportExt;
use crate::ui::{self, Tone};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{Div, SharedString, div, prelude::*, px};
use kubecove_lib::models::ResourceSummary;

pub(super) const EMPTY: &str = "—";
/// `READINESS_NOT_REPORTED` in `src/features/resources/operational-data.ts`.
pub(super) const READINESS_NOT_REPORTED: &str = "Readiness not reported";
/// `RESTART_COUNT_NOT_REPORTED` in `src/features/resources/operational-data.ts`.
pub(super) const RESTART_COUNT_NOT_REPORTED: &str = "Restart count not reported";

#[derive(Clone, Copy)]
pub(super) enum Column {
    Name,
    Status,
    /// The row's Kubernetes context, shown when the workspace has several.
    Context,
    Namespace,
    Kind,
    Ready,
    Restarts,
    Cpu,
    Memory,
    Owner,
    Age,
    Pin,
}

impl Column {
    pub(super) fn label(self) -> &'static str {
        match self {
            Column::Name => "Name",
            Column::Status => "Status",
            Column::Context => "Context",
            Column::Namespace => "Namespace",
            Column::Kind => "Kind",
            Column::Ready => "Ready",
            Column::Restarts => "Restarts",
            Column::Cpu => "CPU",
            Column::Memory => "Memory",
            Column::Owner => "Owner",
            Column::Age => "Age",
            Column::Pin => "",
        }
    }

    /// The sort that this column's header applies. `Owner` and the pin column have none.
    pub(super) fn sort_column(self) -> Option<SortColumn> {
        Some(match self {
            Column::Name => SortColumn::Name,
            Column::Status => SortColumn::Status,
            Column::Context => SortColumn::Context,
            Column::Namespace => SortColumn::Namespace,
            Column::Kind => SortColumn::Kind,
            Column::Ready => SortColumn::Ready,
            Column::Restarts => SortColumn::Restarts,
            Column::Cpu => SortColumn::Cpu,
            Column::Memory => SortColumn::Memory,
            Column::Age => SortColumn::Age,
            Column::Owner | Column::Pin => return None,
        })
    }

    /// `<colgroup>` widths in px; Name takes the remaining width.
    pub(super) fn width(self) -> f32 {
        match self {
            Column::Name => 250.,
            Column::Status => 130.,
            // Sized for "admin@solid-k8s", the longest fixture context name;
            // `tests/text_fit.rs` checks that it shows without an ellipsis.
            Column::Context => 150.,
            Column::Namespace => 105.,
            // Fits "Deployment", the widest default kind: 79.5 px of 14 px Inter
            // plus 20 px cell padding, measured in both clients.
            Column::Kind => 100.,
            Column::Ready => 70.,
            Column::Restarts => 75.,
            Column::Cpu => 65.,
            Column::Memory => 85.,
            Column::Owner => 110.,
            Column::Age => 65.,
            Column::Pin => 40.,
        }
    }
}

/// `resourceReadyChip` in `src/features/resources/helpers.ts`. Only the
/// `true` and `false` readiness values get a chip.
pub(super) fn ready_chip(row: &ResourceSummary) -> Option<(Tone, &'static str)> {
    let ready = row.ready.as_deref()?.trim().to_lowercase();
    match ready.as_str() {
        "true" => Some((Tone::Success, "Ready")),
        "false" => {
            let status = row.status.as_deref().unwrap_or("").trim().to_lowercase();
            let finished = row.kind == "Pod"
                && matches!(status.as_str(), "succeeded" | "complete" | "completed");
            Some(if finished {
                (Tone::Success, "Completed")
            } else {
                (Tone::Error, "Not ready")
            })
        }
        _ => None,
    }
}

pub(super) fn ready_text(row: &ResourceSummary) -> String {
    match ready_chip(row) {
        Some((_, label)) => label.into(),
        None => row.ready.clone().unwrap_or_else(|| EMPTY.into()),
    }
}

pub(super) fn restart_text(row: &ResourceSummary) -> String {
    row.restarts
        .map_or_else(|| EMPTY.into(), |count| count.to_string())
}

/// The element ID that identifies a row's resource in the table. The context
/// comes last, so the same object in two contexts has two IDs and an ID
/// without its context is a prefix of every context's ID.
pub(super) fn resource_id(row: &ResourceSummary) -> String {
    format!(
        "resource:{}:{}:{}:{}",
        row.kind,
        row.namespace.as_deref().unwrap_or(""),
        row.name,
        row.cluster
    )
}
/// Every cell clips to its column, so content cannot draw over a neighbour.
pub(super) fn cell(column: Column, content: impl IntoElement) -> Div {
    let cell = div()
        .flex()
        .items_center()
        .px(px(10.))
        .py(px(12.))
        .child(content);
    match column {
        Column::Name => cell
            .flex_1()
            .min_w(px(Column::Name.width()))
            .overflow_hidden(),
        _ => cell.w(px(column.width())).flex_none().overflow_hidden(),
    }
}

/// Text that truncates with an ellipsis inside its column.
pub(super) fn text_cell(column: Column, value: impl Into<SharedString>) -> Div {
    cell(
        column,
        div()
            .min_w_0()
            .child(ui::sm(value).whitespace_nowrap().truncate()),
    )
}

/// A truncated text cell whose full value is its hover tooltip and accessible
/// description, as `TimestampText` does for timestamps.
pub(super) fn value_cell(column: Column, id: String, value: String) -> Div {
    let tooltip = value.clone();
    cell(
        column,
        div()
            .id(SharedString::from(id))
            .test_support()
            .aria_description(value.clone())
            .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            .min_w_0()
            .child(ui::sm(value).whitespace_nowrap().truncate()),
    )
}
