//! The production workspace overview, matching the web's approved table layout.
use super::resource_filters::HealthFilter;
use super::{Inspector, Load, Page};
use crate::{
    model::HealthSummary,
    targets::TestSupportExt,
    theme::{Palette, Tokens},
    ui,
    workspaces::overview,
};
use gpui_kit::{AnyElement, Context, Div, FontWeight, SharedString, Window, div, prelude::*, px};

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
                .child(
                    ui::muted(ui::text(description.to_owned(), 12., 19.5))
                        .id(SharedString::from(format!("overview-description:{title}")))
                        .test_support()
                        .aria_label(description.to_owned()),
                ),
        )
        .child(div().px(px(12.)).child(content))
}
impl Inspector {
    pub(super) fn overview(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let projection = self.overview_projection();
        let rows: Vec<_> = projection.rows.iter().collect();
        let model = &projection.model;
        let health = projection.health;
        let has_data = matches!(self.overview_load(), Load::Ready) || !rows.is_empty();
        let width = super::resources::page_width(self, window).min(1280.) - 48.;
        // The approved Svelte layout puts the 20rem rail beside the table at @6xl.
        let wide = width >= 1152.;
        let main_width = if wide { width - 336. } else { width };
        let rail = div()
            .flex()
            .gap(px(16.))
            .when(wide, |view| view.flex_col().w(px(320.)).flex_none())
            .when(!wide, |view| view.w_full())
            .child(
                titled_card(
                    "Return to Work",
                    "Pinned and recently opened entry points, with their health now.",
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(ui::semibold(ui::xs("Pinned")))
                        .child(self.entry_list(true, cx))
                        .child(ui::semibold(ui::xs("Recent")))
                        .child(self.entry_list(false, cx))
                        .child(ui::semibold(ui::xs("Shortcuts")))
                        .child(self.saved_shortcuts(cx))
                        .children(self.overview_saved_forwards(cx)),
                )
                .flex_1()
                .min_w_0(),
            )
            .child(
                titled_card(
                    "GitOps",
                    "Argo CD and Flux inventory.",
                    self.gitops_body(cx),
                )
                .flex_1()
                .min_w_0(),
            );
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
                    .flex()
                    .items_end()
                    .justify_between()
                    .gap(px(12.))
                    .pb(px(12.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(ui::caps("Workspace Overview", 11.).line_height(px(16.)))
                            .child(
                                ui::semibold(ui::text(self.workspace_name.clone(), 16., 24.))
                                    .mt(px(4.)),
                            )
                            .child(self.saved_scope_view()),
                    )
                    .child(
                        self.overview_button(
                            "open-resources",
                            "Open resources",
                            true,
                            cx,
                            |this, window, cx| {
                                this.open_overview_resources(None, HealthFilter::All, window, cx)
                            },
                        )
                        .child(ui::icon("boxes", 14., Tokens::foreground()))
                        .child(ui::xs("Open resources")),
                    ),
            )
            .child(self.overview_health(health, has_data, cx))
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .when(!wide, |view| view.flex_col())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(16.))
                            .child(self.overview_findings(&rows, model, has_data, main_width, cx))
                            .children(self.overview_namespaces(model, cx)),
                    )
                    .child(rail),
            )
            .children(self.compare_cards())
    }
    fn overview_health(
        &self,
        health: HealthSummary,
        has_data: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let summary = if has_data {
            format!("{} resources loaded from the saved scope.", health.total)
        } else {
            "Counts appear once workspace resources load.".into()
        };
        let metrics = [
            (
                "Degraded",
                health.degraded,
                HealthFilter::Degraded,
                Some(Palette::red300()),
            ),
            (
                "Needs attention",
                health.attention,
                HealthFilter::Attention,
                Some(Palette::amber300()),
            ),
            (
                "Healthy",
                health.healthy,
                HealthFilter::Healthy,
                Some(Palette::emerald300()),
            ),
            ("Unknown", health.unknown, HealthFilter::Unknown, None),
            (
                "Not evaluated",
                health.not_evaluated,
                HealthFilter::NotEvaluated,
                None,
            ),
            (
                "Restart evidence",
                health.restart_evidence,
                HealthFilter::Restarted,
                None,
            ),
        ];
        let loading = self.loading.phase(
            "overview-health",
            matches!(self.overview_load(), Load::Loading),
        );
        titled_card(
            "Resource Health",
            &summary,
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(div().flex().gap(px(8.)).children(metrics.into_iter().map(
                    |(label, value, filter, tone)| {
                        self.overview_button(
                            format!("overview-health:{label}"),
                            format!("{label} {value}"),
                            has_data && value > 0,
                            cx,
                            move |this, window, cx| {
                                this.open_overview_resources(None, filter, window, cx)
                            },
                        )
                        .flex_1()
                        .min_w_0()
                        .min_h(px(64.))
                        .flex_col()
                        .items_start()
                        .justify_center()
                        .gap(px(2.))
                        .px(px(12.))
                        .child(ui::caps(label, 11.52).line_height(px(18.72)))
                        .child(
                            ui::text(
                                if has_data {
                                    value.to_string()
                                } else {
                                    "-".into()
                                },
                                16.,
                                24.,
                            )
                            .font_weight(FontWeight::BOLD)
                            .text_color(if value > 0 {
                                tone.unwrap_or(Tokens::foreground())
                            } else {
                                Tokens::muted_foreground()
                            }),
                        )
                    },
                )))
                .when(!loading.ready(), |view| {
                    view.child(
                        ui::muted(ui::sm(if loading.shown() {
                            "Loading workspace health..."
                        } else {
                            ""
                        }))
                        .min_h(px(64.)),
                    )
                })
                .when_some(self.workspace_read.as_ref(), |view, state| {
                    view.children(state.warnings.iter().map(|warning| {
                        ui::xs(warning.clone())
                            .id(SharedString::from(format!("overview-warning:{warning}")))
                            .test_support()
                            .aria_label(warning.clone())
                    }))
                })
                .when_some(
                    match self.overview_load() {
                        Load::Failed(error) => Some(error),
                        _ => None,
                    },
                    |view, error| {
                        view.child(self.friendly_failure(
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
                            self.overview_button(
                                "retry-overview",
                                "Retry workspace resources",
                                true,
                                cx,
                                |this, _, cx| this.retry_overview(cx),
                            )
                            .child(ui::xs("Retry workspace resources")),
                        )
                    },
                )
                .when(
                    self.workspace_read
                        .as_ref()
                        .is_some_and(|state| state.retried),
                    |view| {
                        view.child(
                            ui::sm("Workspace resources loaded successfully after retry.")
                                .id("overview-retry-success")
                                .test_support()
                                .aria_label("Workspace resources loaded successfully after retry."),
                        )
                    },
                ),
        )
    }
    fn overview_findings(
        &self,
        rows: &[&kubecove_lib::models::ResourceSummary],
        model: &overview::Overview,
        has_data: bool,
        width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let description = if !has_data {
            "Listed once workspace resources load.".into()
        } else {
            format!(
                "{} findings behind the {} resources counted as degraded or needing attention.",
                model.problems.len(),
                model.problem_resources
            )
        };
        let body: AnyElement = if !model.problems.is_empty() {
            self.overview_problem_table(rows, &model.problems, width, cx)
                .into_any_element()
        } else {
            let partial = matches!(self.overview_load(), Load::Failed(_))
                || self
                    .workspace_read
                    .as_ref()
                    .is_some_and(|read| !read.warnings.is_empty())
                || !self.saved_overview.warnings.is_empty();
            let text = if !has_data {
                ""
            } else if partial {
                "No findings in the part of the scope that loaded. Resource Health lists what did not load."
            } else {
                "No loaded resource is degraded or needs attention. Unknown and not evaluated resources carry no health signal."
            };
            ui::muted(ui::xs(text))
                .id("overview-findings-empty")
                .test_support()
                .aria_label(text)
                .p(px(16.))
                .into_any_element()
        };
        ui::card()
            .id("overview-findings-card")
            .test_support()
            .py(px(12.))
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .px(px(12.))
                    .pb(px(12.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(ui::heading("Needs attention", 14., 20.))
                            .child(ui::muted(ui::text(description, 12., 19.5)).mt(px(4.))),
                    )
                    .child(
                        self.overview_button(
                            "open-incidents",
                            "Open incident queue",
                            true,
                            cx,
                            |this, _, cx| {
                                this.open_incident_filter(crate::incidents::Filter::All, cx)
                            },
                        )
                        .child(ui::xs("Open incident queue")),
                    ),
            )
            .child(body)
    }
    fn overview_saved_forwards(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let count = self
            .launcher
            .active
            .and_then(|id| self.launcher.transfer.documents.get(&id))
            .and_then(|document| document["spec"]["portForwards"].as_array())
            .map_or(0, Vec::len);
        (count > 0).then(|| {
            self.overview_button(
                "overview-saved-forwards",
                format!("{count} saved port forwards"),
                true,
                cx,
                |this, _, cx| this.open_page(Page::PortForwards, cx),
            )
            .child(ui::xs(format!("{count} saved port forwards")))
            .into_any_element()
        })
    }
}
