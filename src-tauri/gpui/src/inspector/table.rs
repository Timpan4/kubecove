//! Resource table panel, mirroring the table in `ResourceBrowser.svelte`
//! (`TABLE_CLASS`, group rows, `ResourceTableStatus` and the pager).

use super::Inspector;
use crate::model::{Entry, group_visual, kind_visual, table_page};
use crate::theme::Tokens;
use crate::ui::{self, Tone};
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, Div, FontWeight, SharedString, div, prelude::*, px};
use kubecove_lib::models::{HealthAssessmentState, ResourceSummary};

const EMPTY: &str = "—";

#[derive(Clone, Copy)]
enum Column {
    Name,
    Status,
    Namespace,
    Kind,
    Ready,
    Restarts,
    // TODO: CPU and Memory columns once the native backend reads metrics.
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
            Column::Owner => "Owner",
            Column::Age => "Age",
            Column::Pin => "",
        }
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
            Column::Owner => 110.,
            Column::Age => 65.,
            Column::Pin => 40.,
        }
    }
}

fn health_badge(row: &ResourceSummary) -> Div {
    let (tone, label) = match row.health_assessment.state {
        HealthAssessmentState::Healthy => (Tone::Success, "Healthy"),
        HealthAssessmentState::NeedsAttention => (Tone::Warning, "Needs attention"),
        HealthAssessmentState::Degraded => (Tone::Error, "Degraded"),
        HealthAssessmentState::Unknown => (Tone::Neutral, "Unknown"),
        HealthAssessmentState::NotEvaluated => (Tone::Neutral, "Not evaluated"),
    };
    ui::status_badge(tone, label)
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
        if rows
            .iter()
            .any(|row| row.git_ops_owner.is_some() || row.argo_app.is_some())
        {
            columns.push(Column::Owner);
        }
        columns.extend([Column::Age, Column::Pin]);
        columns
    }

    fn row_cell(&self, column: Column, row: &ResourceSummary, cx: &mut Context<Self>) -> Div {
        match column {
            Column::Name => {
                let resource = row.clone();
                let id = format!(
                    "resource:{}:{}:{}",
                    row.kind,
                    row.namespace.as_deref().unwrap_or(""),
                    row.name
                );
                cell(
                    column,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .min_w_0()
                        .child(
                            div()
                                .id(SharedString::from(id))
                                .test_support()
                                .aria_label(format!("Open resource {}", row.name))
                                .min_w_0()
                                .cursor_pointer()
                                .child(
                                    ui::medium(ui::sm(row.name.clone()))
                                        .whitespace_nowrap()
                                        .truncate(),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_resource(resource.clone(), window, cx)
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(28.))
                                .child(ui::icon("copy", 12., Tokens::muted_foreground())),
                        ),
                )
            }
            Column::Status => cell(column, health_badge(row)),
            Column::Namespace => text_cell(column, row.namespace.clone().unwrap_or(EMPTY.into())),
            Column::Kind => text_cell(column, row.kind.clone()),
            Column::Ready => text_cell(column, row.ready.clone().unwrap_or(EMPTY.into())),
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
            Column::Owner => text_cell(
                column,
                row.git_ops_owner
                    .as_ref()
                    .map(|owner| owner.name.clone())
                    .or_else(|| row.argo_app.clone())
                    .or_else(|| row.helm_release.clone())
                    .unwrap_or(EMPTY.into()),
            ),
            Column::Age => text_cell(column, row.age.clone()),
            Column::Pin => cell(
                column,
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .child(ui::icon("pin", 14., Tokens::foreground())),
            ),
        }
    }

    /// Group and type rows are ghost buttons (`text-xs/relaxed font-medium`):
    /// groups 12px with `py-2`, types 11px with `py-1.5`; both keep the 1.625 leading.
    fn group_row(
        icon: (&'static str, gpui_kit::Hsla),
        label: String,
        count: String,
        inset: f32,
        strong: bool,
    ) -> Div {
        let (size, count_size) = if strong { (12., 11.) } else { (11., 10.) };
        div()
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
            .child(ui::icon("chevron-down", 16., Tokens::muted_foreground()))
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
        let page = table_page(&self.visible, self.table_page, |row| {
            self.application_manages(row)
        });
        self.table_page = self.table_page.min(page.page_count - 1);
        let columns = self.columns(&page.rows);
        let min_width: f32 = columns.iter().map(|column| column.width()).sum();
        let header = div()
            .flex()
            .bg(Tokens::card())
            .border_b_2()
            .border_color(Tokens::border())
            .children(columns.iter().map(|column| {
                let label = column.label();
                cell(
                    *column,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .when(!label.is_empty(), |head| {
                            head.child(
                                ui::muted(ui::text(label, 11., 16.))
                                    .font_weight(FontWeight::MEDIUM),
                            )
                            .child(ui::icon(
                                if matches!(column, Column::Name) {
                                    "arrow-up"
                                } else {
                                    "chevrons-up-down"
                                },
                                12.,
                                Tokens::muted_foreground(),
                            ))
                        }),
                )
            }));
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
                Entry::Group { label, count } => Self::group_row(
                    group_visual(label),
                    label.clone(),
                    format!("{count} resources on this page"),
                    12.,
                    true,
                )
                .into_any_element(),
                Entry::Type { label, kind, count } => Self::group_row(
                    kind_visual(kind),
                    label.clone(),
                    format!("{count} on this page"),
                    24.,
                    false,
                )
                .into_any_element(),
                Entry::Row(index) => {
                    let row = &page.rows[*index];
                    let cells: Vec<Div> = columns
                        .iter()
                        .map(|column| self.row_cell(*column, row, cx))
                        .collect();
                    div()
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
