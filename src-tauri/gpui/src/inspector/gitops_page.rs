//! GitOps page, mirroring `GitOpsSurface.svelte` and `GitOpsView.svelte`
//! inside `SurfaceFrame`. The page is one virtualized list: the frame header,
//! the provider summary, the rail with the table header and first rows, then
//! the remaining card or list rows. Its connection control opens the shared
//! Argo CD session profile controls.

use super::delivery_summary as summary_ui;
use super::delivery_ui::{self as dui, Layout, ViewMode};
use super::gitops_card;
use super::{Inspector, Page};
use crate::gitops::GitOpsInventory;
use crate::gitops_surface::{self as model, Focus, Provider, Row, Tone};
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{
    AnyElement, Context, Div, ListAlignment, ListState, ScrollHandle, SharedString, Window, div,
    list, prelude::*, px,
};

/// List rows the table puts in the top item, so the rail beside them never
/// runs past it.
const TOP_ROWS: [usize; 2] = [1, 4];

pub(super) struct GitOpsPage {
    pub workspace: Option<super::argo_workspace::ApplicationWorkspace>,
    pub x_scroll: ScrollHandle,
    pub focus: Focus,
    pub mode: ViewMode,
    /// `buildGitOpsSelections` for `focus`, rebuilt when the inventory changes.
    rows: Vec<Row>,
    rows_for: Option<(u64, Focus)>,
    pub layout: Layout,
    list: ListState,
    /// The list's item count and layout, which `reset` remeasures on change.
    shape: Option<(usize, ViewMode, Layout)>,
    /// Sidebar GitOps and Helm nodes expanded by their chevrons, by label.
    pub nav_open: Vec<&'static str>,
}

impl GitOpsPage {
    pub(super) fn new() -> Self {
        Self {
            workspace: None,
            x_scroll: ScrollHandle::new(),
            focus: Focus::Section,
            mode: ViewMode::Cards,
            rows: Vec::new(),
            rows_for: None,
            layout: Layout::new(0., 0),
            list: ListState::new(0, ListAlignment::Top, px(400.)),
            shape: None,
            nav_open: Vec::new(),
        }
    }

    pub(super) fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Card rows, or list rows, for the current layout.
    fn row_count(&self) -> usize {
        match self.mode {
            ViewMode::Cards => self.rows.len().div_ceil(self.layout.columns),
            ViewMode::List => self.rows.len(),
        }
    }

    /// Rows drawn in the top item beside the rail.
    fn top_rows(&self) -> usize {
        if !self.layout.rail_beside {
            return 0;
        }
        let rows = TOP_ROWS[usize::from(self.mode == ViewMode::List)];
        rows.min(self.row_count())
    }
}

/// What one list item draws.
enum Item {
    Header,
    Status,
    Summary,
    Top,
    Rows(usize),
}

impl Inspector {
    /// Opens the GitOps page on a sidebar node, as `selectedNode` does in the web.
    pub(super) fn open_gitops(&mut self, focus: Focus, cx: &mut Context<Self>) {
        if self.page == Page::GitOps && self.gitops_page.focus == focus {
            return;
        }
        if self.page != Page::GitOps {
            self.open_page(Page::GitOps, cx);
            if self.page != Page::GitOps {
                return;
            }
        }
        self.gitops_page.focus = focus;
        self.gitops_page.list.scroll_to(Default::default());
        cx.notify();
    }

    fn sync_gitops_rows(&mut self) {
        let key = (self.gitops.revision, self.gitops_page.focus);
        if self.gitops_page.rows_for == Some(key) {
            return;
        }
        self.gitops_page.rows = self
            .gitops
            .inventory()
            .map(|data| model::rows(data, self.gitops_page.focus))
            .unwrap_or_default();
        self.gitops_page.rows_for = Some(key);
    }

    /// The items the list shows: a header and a status line until the
    /// inventory is ready and has a table to show.
    fn gitops_items(&self) -> usize {
        match self.gitops.inventory() {
            Some(data)
                if data.detected()
                    && model::unavailable(data, self.gitops_page.focus).is_none() =>
            {
                let page = &self.gitops_page;
                3 + page.row_count() - page.top_rows()
            }
            _ => 2,
        }
    }

    fn gitops_item(&self, index: usize) -> Item {
        let page = &self.gitops_page;
        match index {
            0 => Item::Header,
            1 if self.gitops_items() == 2 => Item::Status,
            1 => Item::Summary,
            2 => Item::Top,
            _ => Item::Rows(index - 3 + page.top_rows()),
        }
    }

    pub(super) fn gitops_page_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.sync_gitops_rows();
        let items = self
            .gitops
            .inventory()
            .map_or(0, |_| self.gitops_page.rows.len());
        self.gitops_page.layout = Layout::new(self.frame_width(window), items);
        let count = self.gitops_items();
        let shape = Some((count, self.gitops_page.mode, self.gitops_page.layout));
        if self.gitops_page.shape != shape {
            let mut position = self.gitops_page.list.logical_scroll_top();
            if let Some((_, mode, previous)) = self.gitops_page.shape
                && position.item_ix >= 3
            {
                // Keep the same resource row at the top when opening detail
                // moves the rail or changes the number of card columns.
                let old_columns = if mode == ViewMode::Cards {
                    previous.columns
                } else {
                    1
                };
                let old_rows = items.div_ceil(old_columns);
                let old_top = if previous.rail_beside {
                    TOP_ROWS[usize::from(mode == ViewMode::List)].min(old_rows)
                } else {
                    0
                };
                let resource = (position.item_ix - 3 + old_top) * old_columns;
                let columns = if self.gitops_page.mode == ViewMode::Cards {
                    self.gitops_page.layout.columns
                } else {
                    1
                };
                position.item_ix =
                    3 + (resource / columns).saturating_sub(self.gitops_page.top_rows());
            }
            self.gitops_page.shape = shape;
            self.gitops_page.list.reset(count);
            self.gitops_page.list.scroll_to(position);
        }
        div()
            .id("gitops-list")
            .test_support()
            .size_full()
            .child(
                list(
                    self.gitops_page.list.clone(),
                    cx.processor(|this: &mut Inspector, index: usize, _, cx| {
                        this.gitops_list_item(index, cx)
                    }),
                )
                .size_full(),
            )
            .into_any_element()
    }

    fn gitops_list_item(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let last = index + 1 == self.gitops_items();
        let item = match self.gitops_item(index) {
            Item::Header => self.gitops_header(cx).pt(px(20.)),
            Item::Status => self.gitops_status().pt(px(16.)),
            Item::Summary => self.gitops_summary().pt(px(16.)),
            Item::Top => self.gitops_top(cx).pt(px(16.)),
            Item::Rows(row) => self.gitops_row(row, cx),
        };
        item.w_full()
            .px(px(20.))
            .when(last, |item| item.pb(px(20.)))
            .into_any_element()
    }

    fn gitops_header(&self, cx: &mut Context<Self>) -> Div {
        let argo = self
            .gitops
            .inventory()
            .is_some_and(|data| data.argo.is_some());
        let mut actions = vec![self.live_control(cx).into_any_element()];
        if argo {
            actions.push(
                super::surface::small_button(Some("link-2"), "Argo CD connection")
                    .id("argo-connect-toolbar")
                    .test_support()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.argo_connections.dialog = true;
                        cx.notify();
                    }))
                    .h(px(28.))
                    .into_any_element(),
            );
        }
        dui::frame_header("git-branch", "GitOps", actions)
    }

    fn gitops_status(&self) -> Div {
        let body = match (self.gitops.inventory(), self.gitops.error()) {
            (_, Some(error)) => super::surface::failed("GitOps data unavailable", error.to_owned()),
            (None, None) => super::surface::loading(),
            (Some(data), _) => match model::unavailable(data, self.gitops_page.focus) {
                Some(unavailable) => dui::empty(
                    "gitops-unavailable",
                    unavailable.title,
                    unavailable.description,
                    None,
                )
                .into_any_element(),
                None => dui::empty(
                    "gitops-unavailable",
                    "No GitOps providers detected",
                    "Argo CD and Flux resources were not detected in this cluster.",
                    None,
                )
                .into_any_element(),
            },
        };
        div().child(body)
    }

    fn gitops_summary(&self) -> Div {
        let Some(data) = self.gitops.inventory() else {
            return div();
        };
        let active = model::active_rail_key(data, self.gitops_page.focus);
        let summary = model::summary(data, &active);
        let facts: Vec<summary_ui::Fact> = summary
            .facts
            .iter()
            .map(|fact| summary_ui::Fact {
                label: fact.label,
                value: fact.value,
                color: match fact.tone {
                    Some(Tone::Healthy) => Palette::emerald300(),
                    Some(Tone::Degraded) => Tokens::destructive(),
                    None => Tokens::foreground(),
                },
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .when_some(self.gitops.error().map(str::to_owned), |column, error| {
                column.child(super::surface::failed("GitOps refresh failed", error))
            })
            .when_some(data.list_error.clone(), |column, error| {
                column.child(super::surface::failed(
                    "Some GitOps resources could not load",
                    error,
                ))
            })
            .child(summary_ui::summary(
                "gitops",
                summary.active_provider.label(),
                summary_ui::Badges::Row(
                    summary
                        .detected_providers
                        .iter()
                        .map(|provider| format!("{} detected", provider.label()))
                        .collect(),
                ),
                (summary.total_objects, "GitOps objects"),
                &facts,
                self.gitops_page.layout.summary_columns,
            ))
    }

    fn gitops_rail(&self, data: &GitOpsInventory) -> Div {
        let beside = self.gitops_page.layout.rail_beside;
        let active = model::active_rail_key(data, self.gitops_page.focus);
        let items = model::rail_items(data);
        let sections = [Provider::Argo, Provider::Flux]
            .into_iter()
            .filter_map(|provider| {
                let rows: Vec<AnyElement> = items
                    .iter()
                    .filter(|item| item.provider == provider)
                    .map(|item| {
                        dui::rail_item(&item.label, item.count, item.key == active, item.disabled)
                            .id(SharedString::from(format!("gitops-rail:{}", item.key)))
                            .test_support()
                            .aria_label(format!("{} {}", item.label, item.count))
                            .into_any_element()
                    })
                    .collect();
                (!rows.is_empty()).then(|| dui::rail_section(beside, provider.label(), rows))
            })
            .collect();
        dui::rail(beside, sections)
    }

    /// The table header: title, description, item count and view toggle.
    fn gitops_table_header(&self, data: &GitOpsInventory, cx: &mut Context<Self>) -> Div {
        let table = model::table(data, self.gitops_page.focus);
        let count = self.gitops_page.rows.len();
        let count = format!("{count} item{}", if count == 1 { "" } else { "s" });
        div()
            .flex()
            .flex_wrap()
            .items_end()
            .justify_between()
            .gap(px(12.))
            .pb(px(12.))
            .border_b_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .child(
                        ui::semibold(ui::text(table.title.clone(), 16., 24.))
                            .id("gitops-table-title")
                            .test_support()
                            .aria_label(table.title),
                    )
                    .child(ui::muted(ui::sm(
                        "Delivery state, source, destination, and revision.",
                    ))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        ui::muted(ui::xs(count.clone()))
                            .id("gitops-count")
                            .test_support()
                            .aria_label(count),
                    )
                    .child(dui::view_toggle(
                        "gitops-view",
                        self.gitops_page.mode,
                        |this, mode| this.gitops_page.mode = mode,
                        cx,
                    )),
            )
    }

    fn gitops_top(&self, cx: &mut Context<Self>) -> Div {
        let Some(data) = self.gitops.inventory() else {
            return div();
        };
        let page = &self.gitops_page;
        let mut section = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(16.))
            .child(self.gitops_table_header(data, cx));
        if page.rows.is_empty() {
            let table = model::table(data, page.focus);
            section = section.child(dui::empty(
                "gitops-empty",
                table.empty,
                "Select another GitOps provider or kind from the sidebar.",
                None,
            ));
        } else if page.top_rows() > 0 || page.mode == ViewMode::List {
            let rows: Vec<AnyElement> = (0..page.top_rows())
                .map(|row| self.gitops_row_body(row, cx).into_any_element())
                .collect();
            let closes = page.top_rows() == page.row_count();
            let header = gitops_card::list_header(page.layout.grid - 2., &page.x_scroll);
            section = section.child(dui::rows_block(page.mode, header, rows, closes));
        }
        if page.layout.rail_beside {
            div()
                .flex()
                .gap(px(16.))
                .child(self.gitops_rail(data).self_stretch())
                .child(section)
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(self.gitops_rail(data))
                .child(section)
        }
    }

    /// A row after the top item, beside the rail's empty column.
    fn gitops_row(&self, row: usize, cx: &mut Context<Self>) -> Div {
        let page = &self.gitops_page;
        let last = row + 1 == page.row_count();
        // The first card row under the header takes the section's `space-y-4`.
        let gap = if row == 0 { 16. } else { dui::CARD_GAP };
        let body = dui::continuation(page.mode, self.gitops_row_body(row, cx), last, gap);
        if page.layout.rail_beside {
            div()
                .flex()
                .gap(px(16.))
                .child(dui::rail_spacer().self_stretch())
                .child(body)
        } else {
            div().child(body)
        }
    }

    /// One card row or list row.
    fn gitops_row_body(&self, row: usize, cx: &mut Context<Self>) -> Div {
        let page = &self.gitops_page;
        let Some(data) = self.gitops.inventory() else {
            return div();
        };
        match page.mode {
            ViewMode::Cards => {
                let start = row * page.layout.columns;
                let end = (start + page.layout.columns).min(page.rows.len());
                div().flex().gap(px(dui::CARD_GAP)).children(
                    page.rows[start..end].iter().filter_map(|row| {
                        let selection = row.resolve(data)?;
                        Some(gitops_card::card(selection, page.layout.card, cx))
                    }),
                )
            }
            ViewMode::List => match page.rows.get(row).and_then(|row| row.resolve(data)) {
                Some(selection) => {
                    gitops_card::list_row(selection, page.layout.grid, &page.x_scroll, cx)
                }
                None => div(),
            },
        }
    }
}
