//! Workspace overview, mirroring `src/features/workspaces/WorkspaceOverview.svelte`.

use super::{Inspector, Load, Page};
use crate::model::HealthSummary;
use crate::targets::TestSupportExt;
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::{AnyElement, Context, Div, FontWeight, Hsla, div, prelude::*, px};

/// `Card size="sm"` with a bordered header and its content.
pub(super) fn titled_card(title: &str, description: &str, content: impl IntoElement) -> Div {
    ui::card()
        .py(px(12.))
        .gap(px(12.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .px(px(12.))
                .pb(px(12.))
                .border_b_1()
                .border_color(Tokens::border())
                .child(ui::heading(title.to_owned(), 14., 20.))
                .child(ui::muted(ui::text(description.to_owned(), 12., 19.5))),
        )
        .child(div().px(px(12.)).pt(px(12.)).child(content))
}

fn metric(label: &str, value: usize, tone: Option<Hsla>) -> AnyElement {
    ui::card()
        .id(gpui_kit::SharedString::from(format!(
            "overview-metric:{label}"
        )))
        .test_support()
        .aria_label(format!("{label}: {value}"))
        .flex_1()
        .min_w_0()
        .min_h(px(80.))
        .justify_center()
        .py(px(12.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .px(px(12.))
                // Computed web style: 11.52px with an 18.72px (1.625) line height.
                .child(ui::caps(label, 11.52).line_height(px(18.72)))
                .child(
                    ui::xs(value.to_string())
                        .line_height(px(19.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(tone.unwrap_or(Tokens::foreground())),
                ),
        )
        .into_any_element()
}

fn shortcut(label: &str, count: usize, cx: &mut Context<Inspector>) -> Option<AnyElement> {
    let filter = match label {
        "Degraded" => crate::incidents::Filter::Degraded,
        "Needs attention" => crate::incidents::Filter::Attention,
        _ => crate::incidents::Filter::Restarted,
    };
    (count > 0).then(|| {
        ui::outline_button()
            .id(gpui_kit::SharedString::from(format!(
                "overview-incident:{label}"
            )))
            .test_support()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.open_incident_filter(filter, cx)))
            .h(px(24.))
            .pl(px(6.))
            .pr(px(8.))
            .child(ui::icon("triangle-alert", 12., Tokens::foreground()))
            .child(ui::medium(ui::text(label.to_owned(), 12., 19.5)))
            .child(
                div()
                    .ml(px(4.))
                    .h(px(20.))
                    .px(px(6.))
                    .rounded(px(6.))
                    .bg(Tokens::secondary())
                    .flex()
                    .items_center()
                    .child(ui::medium(ui::text(count.to_string(), 10., 14.))),
            )
            .into_any_element()
    })
}

fn section(title: impl IntoElement, body: impl IntoElement) -> Div {
    ui::panel()
        .overflow_hidden()
        .child(
            div()
                .px(px(12.))
                .py(px(8.))
                .border_b_1()
                .border_color(Tokens::border())
                .child(title),
        )
        .child(body)
}

fn entry_title(icon: &str, title: &str, description: &str) -> Div {
    div()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::icon(icon, 16., Tokens::foreground()))
                .child(ui::semibold(ui::sm(title.to_owned()))),
        )
        .child(ui::muted(ui::xs(description.to_owned())))
}

impl Inspector {
    pub(super) fn overview(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let health = HealthSummary::of(self.workspace_rows());
        let loading = self.loading.phase(
            "overview-health",
            matches!(self.overview_load(), Load::Loading),
        );
        let operations = [
            (
                "open-resources",
                "boxes",
                "Resources",
                Some(Page::Resources),
            ),
            ("open-workspaces", "folder-open", "Workspaces", None),
            (
                "open-port-forwards",
                "cable",
                "Port Forwards",
                Some(Page::PortForwards),
            ),
            (
                "open-incidents",
                "triangle-alert",
                "Incidents",
                Some(Page::Incidents),
            ),
        ]
        .map(|(id, icon, label, destination)| {
            ui::outline_button()
                .id(id)
                .test_support()
                .flex_1()
                .min_w_0()
                .min_h(px(56.))
                .px(px(16.))
                .py(px(12.))
                .gap(px(12.))
                .child(ui::icon(icon, 16., Tokens::foreground()))
                .child(ui::medium(ui::text(label, 12., 19.5)))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| match destination {
                    Some(Page::Incidents) => {
                        this.open_incident_filter(crate::incidents::Filter::All, cx)
                    }
                    // The workspace scope, not the namespace an earlier visit selected, as the
                    // web's `openResources` intent does.
                    Some(Page::Resources) => this.select_namespace(None, window, cx),
                    Some(page) => this.open_page(page, cx),
                    None => this.open_launcher(window, cx),
                }))
                .into_any_element()
        });
        div()
            .id("overview")
            .flex()
            .flex_col()
            .gap(px(16.))
            .w_full()
            .max_w(px(1280.))
            .mx_auto()
            .p(px(24.))
            .when_some(self.workspace_read.as_ref(), |page, state| {
                page.children(state.warnings.iter().map(|warning| {
                    ui::xs(warning.clone())
                        .id(gpui_kit::SharedString::from(format!(
                            "overview-warning:{warning}"
                        )))
                        .test_support()
                        .aria_label(warning.clone())
                }))
            })
            .when_some(
                match self.overview_load() {
                    Load::Failed(error) => Some(error),
                    _ => None,
                },
                |page, error| {
                    page.child(self.friendly_failure(
                        "overview-error",
                        None,
                        error,
                        super::friendly_error::Context {
                            operation: Some(super::friendly_error::Operation::Resources),
                            fallback_title: Some("Failed to refresh workspace resources"),
                            partial: true,
                            ..Default::default()
                        },
                        None,
                        cx,
                    ))
                    .child(
                        ui::outline_button()
                            .id("retry-overview")
                            .test_support()
                            .aria_label("Retry workspace resources")
                            .child("Retry workspace resources")
                            .on_click(cx.listener(|this, _, _, cx| this.retry_overview(cx))),
                    )
                },
            )
            .when(
                self.workspace_read
                    .as_ref()
                    .is_some_and(|state| state.retried),
                |page| {
                    page.child(
                        ui::sm("Workspace resources loaded successfully after retry.")
                            .id("overview-retry-success")
                            .test_support()
                            .aria_label("Workspace resources loaded successfully after retry."),
                    )
                },
            )
            .child(
                div()
                    .pb(px(12.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(ui::caps("Workspace Overview", 11.).line_height(px(16.)))
                    .child(ui::semibold(ui::text(self.workspace_name.clone(), 16., 24.)).mt(px(4.)))
                    .child(self.saved_scope_view()),
            )
            .child(titled_card(
                "Resource Health",
                "Live status across saved workspace scope.",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(metric("Total", health.total, None))
                            .child(metric(
                                "Healthy",
                                health.healthy,
                                Some(Palette::emerald300()),
                            ))
                            .child(metric(
                                "Needs attention",
                                health.attention,
                                Some(Palette::amber300()),
                            ))
                            .child(metric("Degraded", health.degraded, Some(Palette::red300())))
                            .child(metric("Unknown", health.unknown, None))
                            .child(metric("Not evaluated", health.not_evaluated, None)),
                    )
                    .when(!loading.ready(), |content| {
                        content.child(
                            ui::muted(ui::sm(if loading.shown() {
                                "Loading workspace health..."
                            } else {
                                ""
                            }))
                            .flex()
                            .justify_center()
                            .min_h(px(64.)),
                        )
                    })
                    .when(
                        health.degraded + health.attention + health.restart_evidence > 0,
                        |content| {
                            content.child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(px(8.))
                                    .pt(px(12.))
                                    .border_t_1()
                                    .border_color(Tokens::border())
                                    .child(
                                        ui::muted(ui::medium(ui::xs("Incident shortcuts")))
                                            .mr(px(4.)),
                                    )
                                    .children(shortcut("Degraded", health.degraded, cx))
                                    .children(shortcut("Needs attention", health.attention, cx))
                                    .children(shortcut(
                                        "Restart evidence",
                                        health.restart_evidence,
                                        cx,
                                    )),
                            )
                        },
                    ),
            ))
            .child(titled_card(
                "Operations",
                "Open primary workspace surfaces without changing scope.",
                div().flex().gap(px(8.)).children(operations),
            ))
            .child(titled_card(
                "Return to Work",
                "Saved and recent entry points for this workspace.",
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(12.))
                            .child(section(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child(ui::semibold(ui::sm("Shortcuts")))
                                    .child(ui::muted(ui::xs(
                                        self.saved_overview.shortcuts.len().to_string(),
                                    ))),
                                self.saved_shortcuts(cx),
                            ))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(12.))
                                    .child(
                                        section(
                                            entry_title(
                                                "pin",
                                                "Pinned",
                                                "Saved resource entry points.",
                                            ),
                                            self.entry_list(true, cx),
                                        )
                                        .flex_1(),
                                    )
                                    .child(
                                        section(
                                            entry_title(
                                                "clock-3",
                                                "Recent",
                                                "Explicitly opened resources and scopes.",
                                            ),
                                            self.entry_list(false, cx),
                                        )
                                        .flex_1(),
                                    ),
                            ),
                    )
                    .child(
                        section(
                            div()
                                .child(ui::semibold(ui::sm("GitOps")))
                                .child(ui::muted(ui::xs("Argo CD and Flux inventory."))),
                            self.gitops_body(cx),
                        )
                        .w(px(320.))
                        .flex_none(),
                    ),
            ))
            .children(self.compare_cards())
    }
}
