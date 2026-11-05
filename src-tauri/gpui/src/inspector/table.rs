//! Resource table panel, mirroring the table in `ResourceBrowser.svelte`
//! (`TABLE_CLASS`, sortable headers, group rows, `ResourceTableStatus` and the pager).

use super::Inspector;
use crate::model::{
    Entry, TableView, gitops_owner, gitops_owner_name, group_visual, kind_visual, table_page,
};
use crate::table_sort::{self, Column as SortColumn};
use crate::theme::Tokens;
use crate::ui::{self, Tone};
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, Div, FontWeight, SharedString, div, prelude::*, px};
use kubecove_lib::models::{HealthAssessmentState, ResourceSummary};

const EMPTY: &str = "—";
/// `READINESS_NOT_REPORTED` in `src/features/resources/operational-data.ts`.
const READINESS_NOT_REPORTED: &str = "Readiness not reported";
/// `RESTART_COUNT_NOT_REPORTED` in `src/features/resources/operational-data.ts`.
const RESTART_COUNT_NOT_REPORTED: &str = "Restart count not reported";

#[derive(Clone, Copy)]
enum Column {
    Name,
    Status,
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
    fn label(self) -> &'static str {
        match self {
            Column::Name => "Name",
            Column::Status => "Status",
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
    fn sort_column(self) -> Option<SortColumn> {
        Some(match self {
            Column::Name => SortColumn::Name,
            Column::Status => SortColumn::Status,
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
    fn width(self) -> f32 {
        match self {
            Column::Name => 250.,
            Column::Status => 130.,
            Column::Namespace => 105.,
            Column::Kind => 95.,
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

/// `ResourceTableStatus`: the health label and its tone.
fn health(row: &ResourceSummary) -> (Tone, &'static str) {
    match row.health_assessment.state {
        HealthAssessmentState::Healthy => (Tone::Success, "Healthy"),
        HealthAssessmentState::NeedsAttention => (Tone::Warning, "Needs attention"),
        HealthAssessmentState::Degraded => (Tone::Error, "Degraded"),
        HealthAssessmentState::Unknown => (Tone::Neutral, "Unknown"),
        HealthAssessmentState::NotEvaluated => (Tone::Neutral, "Not evaluated"),
    }
}

/// `resourceReadyChip` in `src/features/resources/helpers.ts`. Only the
/// `true` and `false` readiness values get a chip.
fn ready_chip(row: &ResourceSummary) -> Option<(Tone, &'static str)> {
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

fn ready_text(row: &ResourceSummary) -> String {
    match ready_chip(row) {
        Some((_, label)) => label.into(),
        None => row.ready.clone().unwrap_or_else(|| EMPTY.into()),
    }
}

fn restart_text(row: &ResourceSummary) -> String {
    row.restarts
        .map_or_else(|| EMPTY.into(), |count| count.to_string())
}

/// The element ID that identifies a row's resource in the table.
fn resource_id(row: &ResourceSummary) -> String {
    format!(
        "resource:{}:{}:{}",
        row.kind,
        row.namespace.as_deref().unwrap_or(""),
        row.name
    )
}

fn cell(column: Column, content: impl IntoElement) -> Div {
    // Like the web's fixed table layout, only Name truncates; other cells may overflow.
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
        _ => cell.w(px(column.width())).flex_none(),
    }
}

fn text_cell(column: Column, value: impl Into<SharedString>) -> Div {
    cell(column, ui::sm(value).whitespace_nowrap())
}

/// A group or kind-group header row, laid out as its web `Button` is.
struct GroupRow {
    key: String,
    icon: (&'static str, gpui_kit::Hsla),
    label: String,
    count: String,
    inset: f32,
    strong: bool,
    collapsed: bool,
}

impl Inspector {
    fn columns(&self, rows: &[ResourceSummary]) -> Vec<Column> {
        let mut columns = vec![
            Column::Name,
            Column::Status,
            Column::Namespace,
            Column::Kind,
        ];
        if rows.iter().any(|row| row.ready.is_some()) {
            columns.push(Column::Ready);
        }
        if rows.iter().any(|row| row.restarts.is_some()) {
            columns.push(Column::Restarts);
        }
        if rows.iter().any(|row| {
            self.resource_metric(row)
                .and_then(|m| m.cpu_millicores)
                .is_some()
        }) {
            columns.push(Column::Cpu);
        }
        if rows.iter().any(|row| {
            self.resource_metric(row)
                .and_then(|m| m.memory_bytes)
                .is_some()
        }) {
            columns.push(Column::Memory);
        }
        if rows.iter().any(|row| gitops_owner(row).is_some()) {
            columns.push(Column::Owner);
        }
        columns.extend([Column::Age, Column::Pin]);
        columns
    }

    fn usage(&self, row: &ResourceSummary) -> table_sort::Usage {
        let metric = self.resource_metric(row);
        table_sort::Usage {
            cpu_millicores: metric.and_then(|m| m.cpu_millicores),
            memory_bytes: metric.and_then(|m| m.memory_bytes),
        }
    }

    /// The text a cell shows. Each cell exposes it as its accessible label.
    fn cell_text(&self, column: Column, row: &ResourceSummary) -> String {
        match column {
            Column::Name => row.name.clone(),
            Column::Status => health(row).1.into(),
            Column::Namespace => row.namespace.clone().unwrap_or_else(|| EMPTY.into()),
            Column::Kind => row.kind.clone(),
            Column::Ready => ready_text(row),
            Column::Restarts => restart_text(row),
            Column::Cpu => {
                crate::metrics::cpu(self.resource_metric(row).and_then(|m| m.cpu_millicores))
                    .to_string()
            }
            Column::Memory => {
                crate::metrics::memory(self.resource_metric(row).and_then(|m| m.memory_bytes))
                    .to_string()
            }
            Column::Owner => gitops_owner_name(row)
                .or_else(|| row.helm_release.clone())
                .unwrap_or_else(|| EMPTY.into()),
            Column::Age => self.timestamp_label(&row.age, row.created_at.as_deref()),
            Column::Pin => String::new(),
        }
    }

    /// Native equivalent of the web's `title` on values that are not reported.
    fn cell_description(column: Column, row: &ResourceSummary) -> Option<&'static str> {
        match column {
            Column::Ready if ready_chip(row).is_none() => Some(READINESS_NOT_REPORTED),
            Column::Restarts if row.restarts.is_none() => Some(RESTART_COUNT_NOT_REPORTED),
            _ => None,
        }
    }

    fn row_cell(&self, column: Column, row: &ResourceSummary, cx: &mut Context<Self>) -> Div {
        match column {
            Column::Name => cell(
                column,
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .min_w_0()
                    .child(
                        div()
                            .id(SharedString::from(resource_id(row)))
                            .test_support()
                            .aria_label(format!("Open resource {}", row.name))
                            .aria_description(format!("{:?}", row.health_assessment.state))
                            .min_w_0()
                            .cursor_pointer()
                            .child(
                                ui::medium(ui::sm(row.name.clone()))
                                    .whitespace_nowrap()
                                    .truncate(),
                            ),
                    )
                    .child(super::copyable::copy_button(
                        format!(
                            "resource-copy:{}:{}:{}",
                            row.kind,
                            row.namespace.as_deref().unwrap_or(""),
                            row.name
                        ),
                        "resource name",
                        row.name.clone(),
                        cx,
                    )),
            ),
            Column::Status => {
                let (tone, label) = health(row);
                cell(column, ui::status_badge(tone, label))
            }
            Column::Namespace | Column::Kind | Column::Owner | Column::Age => {
                text_cell(column, self.cell_text(column, row))
            }
            Column::Ready => match ready_chip(row) {
                Some((tone, label)) => cell(column, ui::status_badge(tone, label)),
                None => text_cell(column, ready_text(row)),
            },
            Column::Restarts => {
                let content: AnyElement = match row.restarts {
                    None => ui::sm(EMPTY).into_any_element(),
                    Some(0) => ui::sm("0").into_any_element(),
                    Some(count) => ui::status_badge(
                        if count > 5 {
                            Tone::Error
                        } else {
                            Tone::Warning
                        },
                        count.to_string(),
                    )
                    .into_any_element(),
                };
                cell(
                    column,
                    div().flex().w_full().justify_center().child(content),
                )
            }
            Column::Cpu | Column::Memory => {
                let value = self.cell_text(column, row);
                let label = format!(
                    "{} usage {value}",
                    if matches!(column, Column::Cpu) {
                        "CPU"
                    } else {
                        "Memory"
                    }
                );
                cell(
                    column,
                    ui::sm(value)
                        .id(SharedString::from(format!(
                            "metric:{}:{}:{}:{}",
                            row.kind,
                            row.namespace.as_deref().unwrap_or(""),
                            row.name,
                            column.label()
                        )))
                        .test_support()
                        .aria_label(label),
                )
            }
            Column::Pin => {
                let target = row.clone();
                cell(
                    column,
                    div()
                        .id(SharedString::from(format!(
                            "pin:{}:{}:{}",
                            row.kind,
                            row.namespace.as_deref().unwrap_or(""),
                            row.name
                        )))
                        .test_support()
                        .aria_label(format!(
                            "{} {} {}",
                            if self.resource_pinned(row) {
                                "Unpin"
                            } else {
                                "Pin"
                            },
                            row.kind,
                            row.name
                        ))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_resource_pin(&target, cx);
                            cx.stop_propagation();
                        }))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(28.))
                        .child(ui::icon(
                            "pin",
                            14.,
                            if self.resource_pinned(row) {
                                Tokens::primary()
                            } else {
                                Tokens::foreground()
                            },
                        )),
                )
            }
        }
    }

    /// A table cell: the column's content, labeled with its text so that tests
    /// and assistive technology read the value the cell shows.
    fn table_cell(
        &self,
        column: Column,
        row: &ResourceSummary,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = format!("cell:{}:{}", column.label(), resource_id(row));
        let description = Self::cell_description(column, row);
        self.row_cell(column, row, cx)
            .id(SharedString::from(id))
            .test_support()
            .aria_label(self.cell_text(column, row))
            .when_some(description, |cell, description| {
                cell.aria_description(description)
            })
    }

    fn toggle_sort(&mut self, column: SortColumn, cx: &mut Context<Self>) {
        self.table_sort = self.table_sort.toggled(column);
        cx.notify();
    }

    fn toggle_group(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.collapsed_groups.remove(&key) {
            self.collapsed_groups.insert(key);
        }
        cx.notify();
    }

    /// A header cell. Sortable columns are buttons that apply `toggleSort`.
    fn header_cell(&self, column: Column, cx: &mut Context<Self>) -> Div {
        let label = column.label();
        let Some(sort_column) = column.sort_column() else {
            return cell(
                column,
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .when(!label.is_empty(), |head| {
                        head.child(
                            ui::muted(ui::text(label, 11., 16.)).font_weight(FontWeight::MEDIUM),
                        )
                    }),
            );
        };
        let active = self.table_sort.column == sort_column;
        let icon = if active && self.table_sort.desc {
            "arrow-down"
        } else if active {
            "arrow-up"
        } else {
            "chevrons-up-down"
        };
        let aria = match sort_column.description() {
            Some(description) => format!("Sort by {label}. {description}"),
            None => format!("Sort by {label}"),
        };
        cell(
            column,
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .id(SharedString::from(format!("sort:{label}")))
                .test_support()
                .aria_label(aria)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_sort(sort_column, cx)))
                .child(ui::muted(ui::text(label, 11., 16.)).font_weight(FontWeight::MEDIUM))
                .child(ui::icon(icon, 12., Tokens::muted_foreground())),
        )
    }

    /// Group and type rows are ghost buttons (`text-xs/relaxed font-medium`):
    /// groups 12px with `py-2`, types 11px with `py-1.5`; both keep the 1.625 leading.
    fn group_row(&self, row: GroupRow, cx: &mut Context<Self>) -> impl IntoElement {
        let GroupRow {
            key,
            icon,
            label,
            count,
            inset,
            strong,
            collapsed,
        } = row;
        let (size, count_size) = if strong { (12., 11.) } else { (11., 10.) };
        let toggle_key = key.clone();
        div()
            .id(SharedString::from(format!("group-row:{key}")))
            .test_support()
            .aria_label(label.clone())
            .aria_description(if collapsed { "collapsed" } else { "expanded" })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(toggle_key.clone(), cx)))
            .flex()
            .items_center()
            .gap(px(4.))
            .pl(px(inset))
            .pr(px(12.))
            .py(px(if strong { 8. } else { 6. }))
            .border_b_1()
            .border_color(Tokens::border())
            .bg(if strong {
                Tokens::muted().opacity(0.5)
            } else {
                Tokens::card()
            })
            .child(ui::icon(
                if collapsed {
                    "chevron-right"
                } else {
                    "chevron-down"
                },
                16.,
                Tokens::muted_foreground(),
            ))
            .child(ui::icon(icon.0, 14., icon.1).mx(px(2.)))
            .child(
                ui::text(label, size, size * 1.625)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if strong {
                        Tokens::muted_foreground()
                    } else {
                        Tokens::foreground()
                    })
                    .whitespace_nowrap(),
            )
            .child(
                ui::muted(ui::text(count, count_size, size * 1.625))
                    .font_weight(FontWeight::MEDIUM)
                    .ml(px(2.))
                    .whitespace_nowrap(),
            )
    }

    pub(super) fn table_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = TableView {
            page: self.table_page,
            sort: self.table_sort,
            collapsed: &self.collapsed_groups,
            selected: self.detail.resource(),
        };
        let page = table_page(
            &self.visible,
            &view,
            |row| self.application_manages(row),
            &self.persisted.preferences.resource_kind_lanes,
            |row| self.usage(row),
        );
        self.table_page = self.table_page.min(page.page_count - 1);
        let columns = self.columns(&page.rows);
        let min_width: f32 = columns.iter().map(|column| column.width()).sum();
        let header = div()
            .flex()
            .bg(Tokens::card())
            .border_b_2()
            .border_color(Tokens::border())
            .children(columns.iter().map(|column| self.header_cell(*column, cx)));
        let mut body = Vec::new();
        if page.entries.is_empty() {
            body.push(
                div()
                    .id("empty")
                    .test_support()
                    .p(px(32.))
                    .flex()
                    .justify_center()
                    .child(ui::muted(ui::sm(
                        "No resources match current scope and filters.",
                    )))
                    .into_any_element(),
            );
        }
        for entry in &page.entries {
            body.push(match entry {
                Entry::Group {
                    key,
                    label,
                    count,
                    collapsed,
                } => self
                    .group_row(
                        GroupRow {
                            key: key.clone(),
                            icon: group_visual(label),
                            label: label.clone(),
                            count: format!("{count} resources on this page"),
                            inset: 12.,
                            strong: true,
                            collapsed: *collapsed,
                        },
                        cx,
                    )
                    .into_any_element(),
                Entry::Type {
                    key,
                    label,
                    kind,
                    count,
                    collapsed,
                } => self
                    .group_row(
                        GroupRow {
                            key: key.clone(),
                            icon: kind_visual(kind),
                            label: label.clone(),
                            count: format!("{count} on this page"),
                            inset: 24.,
                            strong: false,
                            collapsed: *collapsed,
                        },
                        cx,
                    )
                    .into_any_element(),
                Entry::Row(index) => {
                    let row = &page.rows[*index];
                    let cells: Vec<AnyElement> = columns
                        .iter()
                        .map(|column| self.table_cell(*column, row, cx).into_any_element())
                        .collect();
                    let resource = row.clone();
                    div()
                        .id(SharedString::from(format!("row:{}", resource_id(row))))
                        .test_support()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_resource(resource.clone(), window, cx)
                        }))
                        .flex()
                        .border_b_1()
                        .border_color(Tokens::border())
                        .hover(|row| row.bg(Tokens::muted().opacity(0.6)))
                        .children(cells)
                        .into_any_element()
                }
            });
        }
        let page_index = self.table_page;
        let page_count = page.page_count;
        let pager_button = |id: &'static str, label: &'static str, enabled: bool| {
            ui::outline_button()
                .id(id)
                .test_support()
                .h(px(24.))
                .px(px(8.))
                .when(!enabled, |button| button.opacity(0.5))
                .when(enabled, |button| button.cursor_pointer())
                .child(ui::medium(ui::xs(label)))
        };
        ui::card()
            .size_full()
            .shadow_sm()
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(ui::icon("table-2", 16., Tokens::muted_foreground()))
                                    .child(ui::semibold(ui::sm("Resource Table"))),
                            )
                            .child(
                                ui::muted(ui::xs(format!(
                                    "{} resources · page {} of {}",
                                    page.total,
                                    page_index + 1,
                                    page_count
                                )))
                                .mt(px(2.)),
                            ),
                    )
                    .child(ui::ghost_icon("panel-right-close", 28., 14.)),
            )
            .child(
                div()
                    .id("resource-table")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .child(
                        div()
                            .min_w(px(min_width))
                            .flex()
                            .flex_col()
                            .child(header)
                            .children(body),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(8.))
                    .border_t_1()
                    .border_color(Tokens::border())
                    .child(
                        pager_button("previous-page", "Previous", page_index > 0).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.table_page = this.table_page.saturating_sub(1);
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .child(
                                ui::muted(ui::xs(format!("{} total rows", page.total)))
                                    .flex_1()
                                    .flex()
                                    .justify_center(),
                            )
                            .child(
                                ui::muted(ui::xs(format!(
                                    "Page {} of {}",
                                    page_index + 1,
                                    page_count
                                )))
                                .flex_1()
                                .flex()
                                .justify_center(),
                            ),
                    )
                    .child(
                        pager_button("next-page", "Next", page_index + 1 < page_count).on_click(
                            cx.listener(move |this, _, _, cx| {
                                if this.table_page + 1 < page_count {
                                    this.table_page += 1;
                                    cx.notify();
                                }
                            }),
                        ),
                    ),
            )
    }
}
