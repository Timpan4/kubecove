//! Resource browser, mirroring `ResourceBrowser.svelte` and
//! `ResourceBrowserTopBar.svelte`. GPUI has no container queries, so the
//! `@3xl`/`@5xl` breakpoints use the section width derived from the window.

use super::detail::Detail;
use super::{Inspector, Load, Page};
use crate::model::HealthSummary;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, Div, FontWeight, Hsla, Window, div, prelude::*, px};

const SIDEBAR: f32 = 260.;

fn scope_trigger(label: &str, value: String) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(32.))
        .px(px(10.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.7))
        .child(ui::muted(ui::xs(label.to_owned())))
        .child(ui::semibold(ui::xs(value)).truncate())
        .child(ui::icon("chevron-down", 14., Tokens::muted_foreground()))
}

fn stat(label: &str, value: usize, tone: Option<Hsla>, active: bool) -> AnyElement {
    let color = if value == 0 {
        Tokens::muted_foreground().opacity(0.7)
    } else {
        tone.unwrap_or(Tokens::foreground())
    };
    div()
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(2.))
        .min_w_0()
        .min_h(px(48.))
        .px(px(10.))
        .py(px(6.))
        .rounded(px(8.))
        .border_1()
        .border_color(if active {
            Tokens::primary().opacity(0.5)
        } else {
            Tokens::border()
        })
        // The outline button's `dark:bg-input/30` wins over its base background.
        .bg(Tokens::input().opacity(0.3))
        // The button's `text-xs/relaxed` leading (1.625) also applies to the 10px label.
        .child(ui::caps(label, 10.).line_height(px(16.25)).truncate())
        .child(
            ui::xs(value.to_string())
                .line_height(px(19.5))
                .font_weight(FontWeight::BOLD)
                .text_color(color),
        )
        .into_any_element()
}

fn info_card(title: &str, detail: &str) -> Div {
    ui::panel()
        .flex_1()
        .px(px(12.))
        .py(px(8.))
        .child(ui::semibold(ui::xs(title.to_owned())))
        .child(ui::muted(ui::xs(detail.to_owned())))
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
        main * (1. - detail_share(this.page))
    }
}

/// The detail pane's visible width. `DetailPanelFrame.svelte` sizes the page
/// and the pane as non-shrinking percentages of the frame, so the `w-2` resize
/// handle between them overflows the frame and clips the pane by 8px.
pub(super) fn detail_width(this: &Inspector, window: &Window) -> f32 {
    main_width(window) * detail_share(this.page) - 8.
}

/// The main panel's inner width: the window less the shell's `p-2`, the
/// sidebar, the gap and the panel's border.
fn main_width(window: &Window) -> f32 {
    f32::from(window.viewport_size().width) - 16. - SIDEBAR - 8. - 2.
}

/// `WorkspaceShell.svelte` opens the detail pane at 30% on the GitOps page and
/// 40% elsewhere.
fn detail_share(page: Page) -> f32 {
    if page == Page::GitOps { 0.3 } else { 0.4 }
}

impl Inspector {
    fn section_width(&self, window: &Window) -> f32 {
        page_width(self, window) - 32. - 18.
    }

    pub(super) fn live_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .child(
                ui::outline_button()
                    .id("live-status")
                    .test_support()
                    .aria_label("Live updates and refresh options")
                    .cursor_pointer()
                    // `ResourceLiveStatusMenu.svelte`: `h-8 gap-2 bg-background/50 px-2.5`.
                    .h(px(32.))
                    .px(px(10.))
                    .gap(px(8.))
                    .bg(Tokens::background().opacity(0.5))
                    .child(
                        div()
                            .size(px(6.))
                            .rounded_full()
                            .bg(match self.live.label() {
                                "Live" => super::events::tw::emerald400(1.),
                                "Refresh failed" => Tokens::destructive(),
                                "Catching up" => super::events::tw::sky300(),
                                "Connecting" | "Not connected" => Tokens::muted_foreground(),
                                _ => super::events::tw::amber400(1.),
                            }),
                    )
                    .child(
                        ui::xs(self.live.label())
                            .text_color(match self.live.label() {
                                "Refresh failed" => Tokens::destructive(),
                                "Reconnecting" | "Polling (limited)" | "Live (limited)" => {
                                    Palette::amber300()
                                }
                                _ => Tokens::muted_foreground(),
                            })
                            .id("live-label")
                            .test_support()
                            .aria_label(self.live.label()),
                    )
                    .child(ui::icon("chevron-down", 12., Tokens::muted_foreground()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.live_menu_open = !this.live_menu_open;
                        cx.notify();
                    })),
            )
            .when(self.live_menu_open, |control| {
                control.child(
                    div()
                        .absolute()
                        .top(px(36.))
                        .right_0()
                        .p(px(4.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Tokens::border())
                        .bg(Tokens::card())
                        .shadow_lg()
                        .children(
                            self.live
                                .messages()
                                .map(|message| ui::muted(ui::xs(message.clone()))),
                        )
                        .child(
                            div()
                                .id("refresh")
                                .test_support()
                                .cursor_pointer()
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(6.))
                                .hover(|item| item.bg(Tokens::muted()))
                                .child(ui::xs("Refresh (clear cache)").whitespace_nowrap())
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.refresh(window, cx)),
                                ),
                        ),
                )
            })
    }

    fn top_bar(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let wide = width >= 1024.;
        let health = HealthSummary::of(&self.visible);
        let has_filters = !self.search.read(cx).value().is_empty();
        let namespace = self.scope.namespace.clone().unwrap_or_else(|| "All".into());
        let selectors = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(scope_trigger("Namespace", namespace))
            .child(self.kinds_picker(cx));
        let owners = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(
                ui::outline_button()
                    .h(px(32.))
                    .px(px(8.))
                    .gap(px(8.))
                    .bg(Tokens::background().opacity(0.7))
                    .child(ui::xs("All GitOps owners"))
                    .child(ui::icon("chevron-down", 14., Tokens::muted_foreground())),
            )
            .when(has_filters, |row| {
                row.child(
                    ui::outline_button()
                        .h(px(32.))
                        .px(px(8.))
                        .bg(Tokens::background().opacity(0.7))
                        .child(ui::icon("x", 12., Tokens::foreground()))
                        .child(ui::medium(ui::xs("Clear"))),
                )
            });
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
            stat("Total", health.total, None, true),
            stat(
                "Healthy",
                health.healthy,
                Some(Palette::emerald300()),
                false,
            ),
            stat("Unknown", health.unknown, None, false),
            stat("Not evaluated", health.not_evaluated, None, false),
            stat(
                "Needs attention",
                health.attention,
                Some(Palette::amber300()),
                false,
            ),
            stat("Degraded", health.degraded, Some(Palette::red300()), false),
            stat(
                "Restart evidence",
                health.restart_evidence,
                Some(Palette::sky300()),
                false,
            ),
        ];
        let extras = vec![
            self.metrics_card().into_any_element(),
            info_card("Custom Resources", "Custom Resources added").into_any_element(),
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
            .child(
                div().flex().child(
                    div()
                        .flex_none()
                        .mt(px(8.))
                        .pt(px(8.))
                        .border_t_1()
                        .border_color(Tokens::border())
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .size(px(13.))
                                .rounded(px(3.))
                                .border_1()
                                .border_color(Tokens::muted_foreground())
                                .bg(Tokens::background()),
                        )
                        .child(ui::muted(ui::xs("Hide historical ReplicaSets"))),
                ),
            )
    }

    pub(super) fn resources(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let width = self.section_width(window);
        if self.live.label() != "Refresh failed" {
            self.live.error_open = false;
        }
        let stacked = !matches!(self.detail, Detail::Closed);
        let body: AnyElement = match self.page_load() {
            Load::Loading => ui::muted(ui::xs("Loading resources"))
                .flex()
                .min_h(px(208.))
                .items_center()
                .justify_center()
                .into_any_element(),
            // The top bar stays so the Live menu's refresh can retry the read.
            Load::Failed(error) => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(self.top_bar(width, cx))
                .child(
                    div()
                        .id("load-error")
                        .test_support()
                        .aria_label(error.clone())
                        .role(gpui_kit::Role::Alert)
                        .p(px(12.))
                        .rounded(px(10.))
                        .border_1()
                        .border_color(Tokens::destructive().opacity(0.4))
                        .text_color(Tokens::destructive())
                        .child(ui::xs(error.clone())),
                )
                .into_any_element(),
            _ => {
                let panels = if !self.persisted.preferences.show_ownership_map_by_default {
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(400.))
                        .child(self.table_panel(cx))
                } else if stacked {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(div().h(px(400.)).child(self.map_panel(cx)))
                        .child(div().h(px(560.)).child(self.table_panel(cx)))
                } else {
                    div()
                        .flex()
                        .flex_1()
                        .min_h(px(400.))
                        .gap(px(12.))
                        // `grid-cols-[minmax(0,0.42fr)_minmax(0,0.58fr)] gap-3`: shares apply after the gap.
                        .child(
                            div()
                                .flex_basis(px(0.))
                                .flex_grow(0.42)
                                .min_w_0()
                                .child(self.map_panel(cx)),
                        )
                        .child(
                            div()
                                .flex_basis(px(0.))
                                .flex_grow(0.58)
                                .min_w_0()
                                .child(self.table_panel(cx)),
                        )
                };
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
                            .role(gpui_kit::Role::Alert)
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
