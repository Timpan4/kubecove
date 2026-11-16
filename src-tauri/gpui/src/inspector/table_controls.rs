//! The resource table's controls: sortable headers, group and type rows, and
//! the pin button, mirroring the buttons in `ResourceBrowser.svelte`.

use super::Inspector;
use super::table_columns::{Column, cell};
use crate::table_sort::Column as SortColumn;
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::{Context, Div, FontWeight, SharedString, div, prelude::*, px};
use kubecove_lib::models::ResourceSummary;

/// A group or kind-group header row, laid out as its web `Button` is.
pub(super) struct GroupRow {
    pub key: String,
    pub icon: (&'static str, gpui_kit::Hsla),
    pub label: String,
    pub count: String,
    pub inset: f32,
    pub strong: bool,
    pub collapsed: bool,
}

impl Inspector {
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
    pub(super) fn header_cell(&self, column: Column, cx: &mut Context<Self>) -> Div {
        let label = column.label();
        let Some(sort_column) = column.sort_column() else {
            return cell(
                column,
                div()
                    .id(SharedString::from(format!("head:{label}")))
                    .test_support()
                    .aria_label(label)
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
    pub(super) fn group_row(&self, row: GroupRow, cx: &mut Context<Self>) -> impl IntoElement {
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
                div()
                    .id(SharedString::from(format!("group-count:{key}")))
                    .test_support()
                    .aria_label(count.clone())
                    .child(
                        ui::muted(ui::text(count, count_size, size * 1.625))
                            .font_weight(FontWeight::MEDIUM)
                            .ml(px(2.))
                            .whitespace_nowrap(),
                    ),
            )
    }

    /// The pin column's button, which pins or unpins the row's resource.
    pub(super) fn pin_cell(&self, row: &ResourceSummary, cx: &mut Context<Self>) -> Div {
        let target = row.clone();
        cell(
            Column::Pin,
            div()
                .id(SharedString::from(format!(
                    "pin:{}:{}:{}:{}",
                    row.kind,
                    row.namespace.as_deref().unwrap_or(""),
                    row.name,
                    row.cluster
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
