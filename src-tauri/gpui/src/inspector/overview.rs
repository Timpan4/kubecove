//! Workspace overview, mirroring `src/features/workspaces/WorkspaceOverview.svelte`.

use super::{Inspector, Load, Page};
use crate::model::{HealthSummary, WORKSPACE_KINDS};
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
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

fn shortcut(label: &str, count: usize) -> Option<AnyElement> {
    (count > 0).then(|| {
        ui::outline_button()
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
        let loading = matches!(self.load, Load::Loading);
        let operations = [
            ("open-resources", "boxes", "Resources", true),
            ("open-workspaces", "folder-open", "Workspaces", false),
            ("open-port-forwards", "cable", "Port Forwards", false),
            ("open-incidents", "triangle-alert", "Incidents", false),
        ]
        .map(|(id, icon, label, available)| {
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
                .when(available, |button| {
                    button
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.open_page(Page::Resources, cx)))
                })
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
            .child(
                div()
                    .pb(px(12.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(ui::caps("Workspace Overview", 11.).line_height(px(16.)))
                    .child(ui::semibold(ui::text(self.workspace_name.clone(), 16., 24.)).mt(px(4.)))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(8.))
                            .mt(px(8.))
                            .child(ui::outline_badge(self.scope.context.clone()).rounded(px(6.)))
                            .child(
                                ui::outline_badge(
                                    self.scope
                                        .namespace
                                        .as_ref()
                                        .map_or("All namespaces".into(), |_| {
                                            "1 namespaces".to_owned()
                                        }),
                                )
                                .rounded(px(6.)),
                            )
                            .child(
                                ui::outline_badge(WORKSPACE_KINDS.join(", "))
                                    .rounded(px(6.))
                                    .max_w(px(384.)),
                            ),
                    ),
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
                    .when(loading, |content| {
                        content.child(
                            ui::muted(ui::sm("Loading workspace health..."))
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
                                    .children(shortcut("Degraded", health.degraded))
                                    .children(shortcut("Needs attention", health.attention))
                                    .children(shortcut(
                                        "Restart evidence",
                                        health.restart_evidence,
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
                                    .child(ui::muted(ui::xs("1"))),
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_start()
                                    .min_h(px(64.))
                                    .p(px(12.))
                                    .child(
                                        ui::outline_button()
                                            .id("shortcut-resources")
                                            .test_support()
                                            .cursor_pointer()
                                            .h(px(24.))
                                            .pl(px(6.))
                                            .pr(px(8.))
                                            .child(ui::icon("boxes", 12., Tokens::foreground()))
                                            .child(ui::medium(ui::text("Resources", 12., 19.5)))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.open_page(Page::Resources, cx)
                                            })),
                                    ),
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
                                            ui::muted(ui::sm(
                                                "Pin resources from the table or detail panel.",
                                            ))
                                            .min_h(px(80.))
                                            .p(px(12.)),
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
                                            ui::muted(ui::sm("Recent visits will appear here."))
                                                .min_h(px(80.))
                                                .p(px(12.)),
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
    }
}
