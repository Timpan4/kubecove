//! Resource browser, mirroring `ResourceBrowser.svelte` and
//! `ResourceBrowserTopBar.svelte`. GPUI has no container queries, so the
//! `@3xl`/`@5xl` breakpoints use the section width derived from the window.

use super::detail::Detail;
use super::resource_filters::HealthFilter;
use super::watch_status::LiveStatus;
use super::{Inspector, Load, Page};
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::{AnyElement, Context, Div, SharedString, Window, div, prelude::*, px};

const SIDEBAR: f32 = 260.;

fn info_card(title: &str, detail: &str) -> Div {
    ui::panel()
        .flex_1()
        .px(px(12.))
        .py(px(8.))
        .child(ui::semibold(ui::xs(title.to_owned())))
        .child(ui::muted(ui::xs(detail.to_owned())))
}

/// The web's `Empty` block that replaces the resource table (`ResourceBrowser.svelte`).
fn empty_state(title: &str, description: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .min_h(px(208.))
        .p(px(16.))
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .child(
            div()
                .id("empty-title")
                .test_support()
                .aria_label(title.to_owned())
                .child(ui::semibold(ui::sm(title.to_owned()))),
        )
        .child(
            div()
                .id("empty-description")
                .test_support()
                .aria_label(description.to_owned())
                .child(ui::muted(ui::xs(description.to_owned()))),
        )
        .into_any_element()
}

/// A flex child that takes `share` of the leftover width beside its siblings.
fn grow_panel(share: f32, panel: impl IntoElement) -> Div {
    div()
        .flex_basis(px(0.))
        .flex_grow(share)
        .min_w_0()
        .child(panel)
}

/// Lays out `items` in `columns` equal columns, row by row.
fn grid(items: Vec<AnyElement>, columns: usize, gap: f32) -> Div {
    let mut rows = Vec::new();
    let mut items = items.into_iter().peekable();
    while items.peek().is_some() {
        // `flex_grow` keeps the content height as basis, like CSS grid auto rows
        // sharing extra height equally under `items-stretch`.
        let mut row = div().flex().flex_grow(1.).gap(px(gap));
        for _ in 0..columns {
            row = row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .children(items.next()),
            );
        }
        rows.push(row);
    }
    div().flex().flex_col().gap(px(gap)).children(rows)
}

/// The page column's width: the main panel, less the detail pane when open.
pub(super) fn page_width(this: &Inspector, window: &Window) -> f32 {
    let main = main_width(window);
    if matches!(this.detail, Detail::Closed) {
        main
    } else {
        main * (1. - detail_share(this))
    }
}

/// The detail pane's visible width. `DetailPanelFrame.svelte` sizes the page
/// and the pane as non-shrinking percentages of the frame, so the `w-2` resize
/// handle between them overflows the frame and clips the pane by 8px.
pub(super) fn detail_width(this: &Inspector, window: &Window) -> f32 {
    main_width(window) * detail_share(this) - 8.
}

/// The main panel's inner width: the window less the shell's `p-2`, the
/// sidebar, the gap and the panel's border.
fn main_width(window: &Window) -> f32 {
    f32::from(window.viewport_size().width) - 16. - SIDEBAR - 8. - 2.
}

/// `DETAIL_PANEL_DEFAULT_SIZE` (40), or 30 on GitOps, where `WorkspaceShell.svelte`
/// passes its own default.
fn detail_default(page: Page) -> f32 {
    if page == Page::GitOps { 30. } else { 40. }
}

/// `DETAIL_PANEL_MIN_SIZE` (33), or 25 on GitOps (`WorkspaceShell.svelte`).
fn detail_min(page: Page) -> f32 {
    if page == Page::GitOps { 25. } else { 33. }
}

/// `100 - MAIN_PANEL_MIN_SIZE` from `detailPanelFrameModel.ts`.
const DETAIL_MAX: f32 = 70.;

/// `DETAIL_PANEL_RESIZE_STEP` from `detailPanelFrameModel.ts`.
const DETAIL_RESIZE_STEP: f32 = 2.;

/// The detail pane's percent of the frame.
pub(super) fn detail_percent(this: &Inspector) -> f32 {
    this.detail_size
        .unwrap_or_else(|| detail_default(this.page))
}

fn detail_share(this: &Inspector) -> f32 {
    detail_percent(this) / 100.
}

/// The two collapsible panels of the resources page.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Panel {
    Map,
    Table,
}

impl Inspector {
    /// The map is open unless the user collapsed it. Without a choice, it follows
    /// "Show ownership map by default".
    pub(super) fn map_is_open(&self) -> bool {
        self.map_open
            .unwrap_or(self.persisted.preferences.show_ownership_map_by_default)
    }

    pub(super) fn set_panel_open(&mut self, panel: Panel, open: bool) {
        match panel {
            Panel::Map => {
                self.map_open = Some(open);
                // The web unmounts `OwnershipMap.svelte` when it collapses, which
                // forgets the kind groups the user expanded.
                if !open {
                    self.map.expanded.clear();
                    self.rebuild_map();
                }
            }
            Panel::Table => self.table_open = open,
        }
    }

    /// Changes the page. The web's `DetailPanelFrame` resets its size whenever
    /// `sizeKey` changes between GitOps and everything else, so a size dragged on
    /// one never returns on the other.
    pub(super) fn set_page(&mut self, page: Page) {
        if (self.page == Page::GitOps) != (page == Page::GitOps) {
            self.detail_size = None;
        }
        self.request_view();
        self.page = page;
    }

    /// Sets the detail pane's percent, clamped as `clampDetailPanelSize` does.
    pub(super) fn set_detail_percent(&mut self, percent: f32) {
        let size = percent.clamp(detail_min(self.page), DETAIL_MAX);
        self.detail_size = Some(size);
    }

    /// `detailPanelSizeFromPointer`: the frame's right edge sits at the window's
    /// right padding and border, which `main_width` also assumes.
    pub(super) fn detail_percent_at(&self, window: &Window, x: f32) -> f32 {
        let width = main_width(window);
        if width <= 0. {
            return detail_default(self.page);
        }
        let right = f32::from(window.viewport_size().width) - 9.;
        (right - x) / width * 100.
    }

    /// `handleResizeKeydown`: Left widens the pane, Right narrows it, Home and End
    /// go to the limits. Returns whether the key resized the pane.
    pub(super) fn resize_detail_key(&mut self, key: &str) -> bool {
        let current = detail_percent(self);
        let next = match key {
            "left" => current + DETAIL_RESIZE_STEP,
            "right" => current - DETAIL_RESIZE_STEP,
            "home" => detail_min(self.page),
            "end" => DETAIL_MAX,
            _ => return false,
        };
        self.set_detail_percent(next);
        true
    }

    /// A panel's collapsed state: a 3rem strip with a restore button and the
    /// panel's name, as `ResourceBrowser.svelte` renders it.
    fn panel_strip(&self, panel: Panel, cx: &mut Context<Self>) -> AnyElement {
        let (label, icon, control) = match panel {
            Panel::Map => ("Map", "git-branch", "Show ownership map"),
            Panel::Table => ("Table", "table-2", "Show resource table"),
        };
        let show = move |this: &mut Self,
                         _: &gpui_kit::ClickEvent,
                         _: &mut Window,
                         cx: &mut Context<Self>| {
            this.set_panel_open(panel, true);
            cx.notify();
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .flex_none()
            .w(px(48.))
            .h_full()
            .min_h(px(400.))
            .overflow_hidden()
            .rounded(px(10.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .shadow_sm()
            .child(
                div()
                    .flex()
                    .w_full()
                    .justify_center()
                    .p(px(8.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        ui::ghost_icon("panel-right-open", 28., 14.)
                            .id(SharedString::from(format!("panel-control:{control}")))
                            .test_support()
                            .aria_label(control)
                            .cursor_pointer()
                            .on_click(cx.listener(show)),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("panel-strip:{label}")))
                    .test_support()
                    .aria_label(control)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .items_center()
                    .gap(px(8.))
                    .py(px(12.))
                    .cursor_pointer()
                    .text_color(Tokens::muted_foreground())
                    .hover(|strip| strip.bg(Tokens::muted()))
                    .child(ui::icon(icon, 16., Tokens::muted_foreground()))
                    .children(
                        label
                            .chars()
                            .map(|letter| ui::semibold(ui::xs(letter.to_string()))),
                    )
                    .on_click(cx.listener(show)),
            )
            .into_any_element()
    }

    /// The map and table panels. Both open share the page as `0.42fr` and
    /// `0.58fr`, or stack when the detail pane is open. A collapsed panel leaves
    /// its strip beside the other panel.
    fn panels(&mut self, stacked: bool, cx: &mut Context<Self>) -> AnyElement {
        match (self.map_is_open(), self.table_open) {
            (true, true) if stacked => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(div().h(px(400.)).child(self.map_panel(cx)))
                .child(div().h(px(560.)).child(self.table_panel(cx)))
                .into_any_element(),
            (map_open, table_open) => {
                let map: AnyElement = if map_open {
                    let share = if table_open { 0.42 } else { 1. };
                    grow_panel(share, self.map_panel(cx)).into_any_element()
                } else {
                    self.panel_strip(Panel::Map, cx)
                };
                let table: AnyElement = if table_open {
                    let share = if map_open { 0.58 } else { 1. };
                    grow_panel(share, self.table_panel(cx)).into_any_element()
                } else {
                    self.panel_strip(Panel::Table, cx)
                };
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(400.))
                    .gap(px(12.))
                    .child(map)
                    .child(table)
                    .into_any_element()
            }
        }
    }
}

impl Inspector {
    fn section_width(&self, window: &Window) -> f32 {
        page_width(self, window) - 32. - 18.
    }

    fn top_bar(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let wide = width >= 1024.;
        let health = &self.filters.summary;
        let has_filters = self.filters_active(cx);
        let selectors = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(self.namespace_picker(cx))
            .children(self.context_picker(cx))
            .child(self.kinds_picker(cx));
        let owners = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .children((!self.filters.owners.is_empty()).then(|| self.gitops_owner_select(cx)))
            .when(has_filters, |row| row.child(self.clear_filters_button(cx)));
        let controls = if wide {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(selectors)
                .child(div().flex_1().min_w(px(288.)).child(self.search_input()))
                .child(owners)
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(selectors)
                .child(self.search_input())
                .child(owners)
        };
        let stats = vec![
            self.health_stat(HealthFilter::All, "Total", health.total, None, cx),
            self.health_stat(
                HealthFilter::Healthy,
                "Healthy",
                health.healthy,
                Some(Palette::emerald300()),
                cx,
            ),
            self.health_stat(HealthFilter::Unknown, "Unknown", health.unknown, None, cx),
            self.health_stat(
                HealthFilter::NotEvaluated,
                "Not evaluated",
                health.not_evaluated,
                None,
                cx,
            ),
            self.health_stat(
                HealthFilter::Attention,
                "Needs attention",
                health.attention,
                Some(Palette::amber300()),
                cx,
            ),
            self.health_stat(
                HealthFilter::Degraded,
                "Degraded",
                health.degraded,
                Some(Palette::red300()),
                cx,
            ),
            self.health_stat(
                HealthFilter::Restarted,
                "Restart evidence",
                health.restart_evidence,
                Some(Palette::sky300()),
                cx,
            ),
        ];
        let extras = vec![
            self.metrics_card().into_any_element(),
            info_card(
                "Custom Resources",
                self.custom_status().unwrap_or("available"),
            )
            .into_any_element(),
            div()
                .flex()
                .items_center()
                .justify_end()
                .child(self.live_control(cx))
                .into_any_element(),
        ];
        let summary = grid(stats, if width >= 768. { 4 } else { 2 }, 8.);
        let extras = grid(extras, if width >= 768. { 2 } else { 1 }, 8.);
        let health_row = if wide {
            div()
                .flex()
                .gap(px(8.))
                .child(summary.flex_1().min_w_0())
                .child(extras.flex_1().min_w_0())
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(summary)
                .child(extras)
        };
        div()
            .flex_none()
            .p(px(8.))
            .rounded(px(10.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .shadow_sm()
            .child(controls)
            .child(
                health_row
                    .mt(px(8.))
                    .pt(px(8.))
                    .border_t_1()
                    .border_color(Tokens::border()),
            )
            .children(self.application_resource_filters(cx))
            // `w-fit`: the separator spans only the checkbox label.
            .child(self.historical_toggle(cx))
    }

    /// The web's empty states, checked before the load state: no cluster
    /// context, then no selected kind. A failed read still reports its error.
    fn empty_scope(&self) -> Option<AnyElement> {
        if self.scope.context.is_empty()
            && !matches!(self.page_load(), Load::Loading | Load::Failed(_))
        {
            return Some(empty_state(
                "No cluster context",
                "Select a workspace with a cluster context.",
            ));
        }
        let no_kinds = self.live.selected.is_none()
            && self
                .kind_selection
                .chosen
                .as_ref()
                .is_some_and(|chosen| chosen.is_empty());
        if no_kinds {
            return Some(empty_state(
                "No resource scope",
                "Select at least one kind.",
            ));
        }
        None
    }

    pub(super) fn resources(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let width = self.section_width(window);
        if self.resource_live_status() != LiveStatus::RefreshFailed {
            self.live.error_open = false;
        }
        let stacked = !matches!(self.detail, Detail::Closed);
        self.rebuild_map();
        self.sync_map_motion(window, cx);
        self.sync_map_focus(window, cx);
        let loading = self
            .loading
            .phase("resources", matches!(self.page_load(), Load::Loading));
        let body: AnyElement = match self.empty_scope() {
            Some(empty) => empty,
            None => match self.page_load() {
                Load::Loading | Load::Ready if !loading.ready() => div()
                    .flex()
                    .min_h(px(208.))
                    .items_center()
                    .justify_center()
                    .when(loading.shown(), |slot| {
                        slot.child(
                            div()
                                .id("resources-loading")
                                .test_support()
                                .aria_label("Loading resources")
                                .child(ui::muted(ui::xs("Loading resources"))),
                        )
                    })
                    .into_any_element(),
                // The top bar stays so the Live menu's refresh can retry the read.
                Load::Failed(error) => div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(self.top_bar(width, cx))
                    .child(self.friendly_failure(
                        "load-error",
                        None,
                        error,
                        super::friendly_error::Context {
                            operation: Some(super::friendly_error::Operation::Resources),
                            fallback_title: Some("Failed to load resources"),
                            ..Default::default()
                        },
                        None,
                        cx,
                    ))
                    .into_any_element(),
                _ => {
                    let panels = self.panels(stacked, cx);
                    div()
                        .flex()
                        .flex_col()
                        .when(!stacked, |body| body.flex_1().min_h_0())
                        .gap(px(12.))
                        .child(self.top_bar(width, cx))
                        .children(self.live_notice(cx))
                        .children(self.page_warnings().enumerate().map(|(index, warning)| {
                            div()
                                .id(("warning", index))
                                .test_support()
                                .role(gpui_kit::Role::Alert)
                                .aria_label(warning.clone())
                                .px(px(12.))
                                .py(px(8.))
                                .rounded(px(8.))
                                .border_1()
                                .border_color(Palette::amber500(0.3))
                                .text_color(Palette::amber300())
                                .child(ui::xs(warning.clone()))
                        }))
                        .child(panels)
                        .into_any_element()
                }
            },
        };
        div()
            .id("resources")
            .test_support()
            .flex()
            .flex_col()
            // Stacked beside the detail panel, the page keeps its content height
            // so the scrolling page reaches the table, as the web pane does.
            .when(!stacked, |page| page.h_full().min_h_0())
            .when(stacked, |page| page.flex_none())
            .p(px(16.))
            .gap(px(12.))
            .children(self.application_workspace_header(cx))
            .child(body)
    }
}
