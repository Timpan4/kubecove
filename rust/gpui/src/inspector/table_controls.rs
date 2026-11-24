//! The resource table's controls: sortable headers, group and type rows, and
//! the pin button, mirroring the buttons in `ResourceBrowser.svelte`. Each one
//! tracks a tab-stop handle from `table_tab_stop`, so Tab reaches it and Enter
//! or Space activates it, as the web's native buttons do.

use super::Inspector;
use super::table::RowHandle;
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

/// The element id of a column's sort header.
pub(super) fn sort_id(column: Column) -> String {
    format!("sort:{}", column.label())
}

/// The element id of a group or type row.
pub(super) fn group_row_id(key: &str) -> String {
    format!("group-row:{key}")
}

/// The element id of the pin button of the row whose `resource_id` is `id`. It
/// ends in the row's context, as the row's id does.
pub(super) fn pin_id_of(id: &str) -> String {
    format!("pin:{}", id.strip_prefix("resource:").unwrap_or(id))
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
        let id = sort_id(column);
        let focus = self.table_tab_stop(&id, cx);
        cell(
            column,
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .id(SharedString::from(id))
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label(aria)
                .track_focus(&focus)
                .rounded(px(4.))
                .focus(|style| style.bg(Tokens::muted()))
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
        let id = group_row_id(&key);
        let focus = self.table_tab_stop(&id, cx);
        div()
            .id(SharedString::from(id))
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(label.clone())
            .aria_description(if collapsed { "collapsed" } else { "expanded" })
            .track_focus(&focus)
            .focus(|style| style.bg(Tokens::muted()))
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
            // The web's ghost `Button` fades to `hover:bg-muted/60` over
            // `bg-muted/50`, or to `hover:bg-muted/40`. The fill lies over the
            // row's own background, which a focused row replaces, so it adds
            // only the difference.
            .child(
                crate::control_motion::hover_fill(
                    gpui_kit::transparent_black(),
                    Tokens::muted().opacity(if strong { 0.2 } else { 0.4 }),
                )
                .under_border(0., 0., 1., 0.),
            )
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
    pub(super) fn pin_cell(
        &self,
        row: &ResourceSummary,
        handle: &RowHandle,
        cx: &mut Context<Self>,
    ) -> Div {
        let target = handle.clone();
        let id = pin_id_of(&handle.id);
        let focus = self.table_tab_stop(&id, cx);
        let pinned = self.resource_pinned(row);
        cell(
            Column::Pin,
            div()
                .id(SharedString::from(id))
                .test_support()
                .role(gpui_kit::Role::Button)
                .track_focus(&focus)
                .rounded(px(6.))
                .border_1()
                .border_color(gpui_kit::transparent_black())
                .focus(|style| style.border_color(Tokens::ring()))
                .aria_label(format!(
                    "{} {} {}",
                    if pinned { "Unpin" } else { "Pin" },
                    row.kind,
                    row.name
                ))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(resource) = this.table_row(&target) {
                        this.toggle_resource_pin(&resource, cx);
                    }
                    cx.stop_propagation();
                }))
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.))
                .child(ui::icon(
                    "pin",
                    14.,
                    if pinned {
                        Tokens::primary()
                    } else {
                        Tokens::foreground()
                    },
                )),
        )
    }
}
