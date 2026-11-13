use super::{Inspector, overview::titled_card, resource_filters::HealthFilter};
use crate::{
    targets::TestSupportExt,
    theme::{MONO, Palette, Tokens},
    ui,
    workspaces::overview::{Namespace, Overview, Problem, application_owner},
};
use gpui_kit::{Context, Div, SharedString, div, prelude::*, px};
use kubecove_lib::models::{HealthAssessmentState, ResourceSummary};

enum FindingRow<'a> {
    Root {
        resource: &'a ResourceSummary,
        cause: &'a ResourceSummary,
        problem: &'a Problem,
        open: bool,
    },
    Child(&'a ResourceSummary),
}

fn identity(row: &ResourceSummary) -> String {
    format!(
        "{}:{}:{}:{}",
        row.kind,
        row.namespace.as_deref().unwrap_or(""),
        row.name,
        row.cluster
    )
}
fn cell(width: f32, content: impl IntoElement) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .px(px(8.))
        .py(px(8.))
        .min_w_0()
        .child(content)
}
impl Inspector {
    pub(super) fn overview_problem_table(
        &self,
        rows: &[&ResourceSummary],
        problems: &[Problem],
        width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let width = width.max(640.);
        let headers = [
            "Workload / resource",
            "Status",
            "Ready",
            "Restarts",
            "GitOps app",
        ];
        let mut body = div().w(px(width)).flex().flex_col().child(
            div()
                .flex()
                .border_b_1()
                .border_color(Tokens::border())
                .children(
                    headers
                        .into_iter()
                        .zip([0.34, 0.33, 0.09, 0.11, 0.13])
                        .map(|(label, share)| cell(width * share, ui::muted(ui::xs(label)))),
                ),
        );
        for problem in problems {
            let row = rows[problem.root];
            let key = identity(row);
            let open = self.overview_ui.expanded.contains(&key);
            body = body.child(self.overview_problem_row(
                FindingRow::Root {
                    resource: row,
                    cause: rows[problem.cause],
                    problem,
                    open,
                },
                width,
                cx,
            ));
            if open {
                for &index in &problem.related {
                    body = body.child(self.overview_problem_row(
                        FindingRow::Child(rows[index]),
                        width,
                        cx,
                    ));
                }
            }
        }
        div()
            .id("overview-problems")
            .test_support()
            .overflow_x_scroll()
            .child(body)
    }
    fn overview_problem_row(
        &self,
        finding: FindingRow<'_>,
        width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let (row, cause, state, related, open, child) = match finding {
            FindingRow::Root {
                resource,
                cause,
                problem,
                open,
            } => (
                resource,
                cause,
                problem.state,
                problem.related.len(),
                open,
                false,
            ),
            FindingRow::Child(resource) => (
                resource,
                resource,
                resource.health_assessment.state,
                0,
                false,
                true,
            ),
        };
        let key = identity(row);
        let status = cause.status.as_deref().unwrap_or("Unknown");
        let ready = row.ready.as_deref().unwrap_or("-");
        let target = row.clone();
        let color = if state == HealthAssessmentState::Degraded {
            Palette::red300()
        } else {
            Palette::amber300()
        };
        let mut name = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                self.overview_button(
                    format!("overview-open:{key}"),
                    format!(
                        "Open {} {} in {} on {}",
                        row.kind,
                        row.name,
                        row.namespace.as_deref().unwrap_or("cluster scope"),
                        row.cluster
                    ),
                    true,
                    cx,
                    move |this, window, cx| {
                        if !this.can_leave(cx) {
                            return;
                        }
                        this.open_page(super::Page::Resources, cx);
                        this.open_resource(target.clone(), window, cx);
                    },
                )
                .border_0()
                .bg(gpui_kit::transparent_black())
                .px_0()
                .child(
                    ui::xs(row.name.clone())
                        .font_family(MONO)
                        .text_color(Tokens::foreground()),
                ),
            )
            .child(ui::muted(ui::xs(if child {
                row.kind.clone()
            } else {
                format!(
                    "{} in {}{}",
                    row.kind,
                    row.namespace.as_deref().unwrap_or("cluster scope"),
                    if self.workspace_contexts().len() > 1 {
                        format!(" on {}", row.cluster)
                    } else {
                        String::new()
                    }
                )
            })));
        if related > 0 {
            let toggled = key.clone();
            name = name.child(
                self.overview_button(
                    format!("overview-expand:{key}"),
                    format!(
                        "Owned resources of {} in {} on {}",
                        row.name,
                        row.namespace.as_deref().unwrap_or(""),
                        row.cluster
                    ),
                    true,
                    cx,
                    move |this, _, cx| {
                        if !this.overview_ui.expanded.remove(&toggled) {
                            this.overview_ui.expanded.insert(toggled.clone());
                        }
                        cx.notify();
                    },
                )
                .aria_description(if open { "expanded" } else { "collapsed" })
                .border_0()
                .bg(gpui_kit::transparent_black())
                .px_0()
                .child(ui::icon(
                    if open {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    12.,
                    Tokens::muted_foreground(),
                ))
                .child(ui::muted(ui::xs(format!(
                    "{related} owned resource{} also failing",
                    if related == 1 { "" } else { "s" }
                )))),
            );
        }
        let mut status_cell = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().flex().child(super::delivery_ui::badge(
                (color.opacity(0.4), Tokens::input().opacity(0.3), color),
                status.to_owned(),
            )))
            .children(
                cause
                    .health_assessment
                    .reasons
                    .first()
                    .map(|reason| ui::muted(ui::xs(reason.clone()))),
            );
        if identity(cause) != key {
            let target = cause.clone();
            status_cell = status_cell
                .child(ui::muted(ui::xs(format!("From {}", cause.kind))))
                .child(
                    self.overview_button(
                        format!("overview-cause:{key}"),
                        format!("Open {} {}", cause.kind, cause.name),
                        true,
                        cx,
                        move |this, window, cx| {
                            if !this.can_leave(cx) {
                                return;
                            }
                            this.open_page(super::Page::Resources, cx);
                            this.open_resource(target.clone(), window, cx);
                        },
                    )
                    .border_0()
                    .bg(gpui_kit::transparent_black())
                    .px_0()
                    .child(ui::xs(cause.name.clone()).font_family(MONO)),
                );
        }
        let owner = row.git_ops_owner.as_ref();
        let app = owner
            .map(|owner| owner.name.as_str())
            .or(row.argo_app.as_deref());
        let application = application_owner(row);
        let app_cell = if let Some(app) = app.filter(|_| {
            (application.is_some() || row.cluster == self.scope.context)
                && owner.is_none_or(|owner| owner.provider == "argo" && owner.kind == "Application")
        }) {
            let app = app.to_owned();
            self.overview_button(
                format!("overview-app:{key}"),
                format!("Open application {app}"),
                true,
                cx,
                move |this, window, cx| {
                    if !this.can_leave(cx) {
                        return;
                    }
                    if let Some(target) = application.clone() {
                        this.open_page(super::Page::Resources, cx);
                        this.open_resource(target, window, cx);
                    } else {
                        this.open_page(super::Page::GitOps, cx);
                    }
                },
            )
            .border_0()
            .bg(gpui_kit::transparent_black())
            .px_0()
            .child(ui::xs(app).font_family(MONO))
            .into_any_element()
        } else {
            ui::muted(ui::xs(app.unwrap_or("None").to_owned())).into_any_element()
        };
        div()
            .id(SharedString::from(format!("overview-problem:{key}")))
            .test_support()
            .aria_label(format!(
                "{} / {} in {} on {}",
                row.kind,
                row.name,
                row.namespace.as_deref().unwrap_or("cluster scope"),
                row.cluster
            ))
            .aria_description(format!("{status}|{ready}"))
            .flex()
            .items_start()
            .border_b_1()
            .border_color(Tokens::border())
            .when(child, |view| view.bg(Tokens::muted().opacity(0.2)))
            .child(cell(
                width * 0.34,
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .when(child, |view| view.pl(px(16.)))
                    .when(!child, |view| {
                        view.child(
                            div()
                                .size(px(8.))
                                .mt(px(6.))
                                .rounded_full()
                                .flex_none()
                                .bg(color),
                        )
                    })
                    .child(name.min_w_0()),
            ))
            .child(cell(width * 0.33, status_cell))
            .child(cell(width * 0.09, ui::xs(ready.to_owned()).text_right()))
            .child(cell(
                width * 0.11,
                ui::xs(cause.restarts.unwrap_or(0).to_string()).text_right(),
            ))
            .child(cell(width * 0.13, app_cell))
    }
    pub(super) fn overview_namespaces(
        &self,
        model: &Overview,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if model.namespaces.is_empty() {
            return None;
        }
        let findings = |row: &&Namespace| row.degraded + row.attention > 0;
        let count = model.namespaces.iter().filter(findings).count();
        let quiet = model.namespaces.len() - count;
        let open = self.overview_ui.quiet || count == 0;
        let mut body = div().flex().flex_col().gap(px(4.));
        if quiet > 0 && count > 0 {
            body = body.child(
                self.overview_button(
                    "overview-quiet",
                    format!(
                        "{} {quiet} without findings",
                        if open { "Hide" } else { "Show" }
                    ),
                    true,
                    cx,
                    |this, _, cx| {
                        this.overview_ui.quiet = !this.overview_ui.quiet;
                        cx.notify();
                    },
                )
                .child(ui::xs(format!(
                    "{} {quiet} without findings",
                    if open { "Hide" } else { "Show" }
                ))),
            );
        }
        for row in &model.namespaces {
            if !open && row.degraded + row.attention == 0 {
                continue;
            }
            let namespace = row.name.clone();
            let health = if row.degraded > 0 {
                HealthFilter::Degraded
            } else if row.attention > 0 {
                HealthFilter::Attention
            } else {
                HealthFilter::All
            };
            body = body.child(
                self.overview_button(
                    format!("overview-namespace:{}", row.name),
                    format!("Open namespace {}", row.name),
                    true,
                    cx,
                    move |this, window, cx| {
                        this.open_overview_resources(Some(namespace.clone()), health, window, cx)
                    },
                )
                .border_0()
                .bg(gpui_kit::transparent_black())
                .justify_between()
                .child(ui::xs(row.name.clone()).font_family(MONO))
                .child(ui::muted(ui::xs(format!(
                    "{} degraded / {} attention of {}",
                    row.degraded, row.attention, row.total
                )))),
            );
        }
        Some(titled_card(
            "Namespaces",
            &format!(
                "{count} namespaces with findings, of {} loaded.",
                model.namespaces.len()
            ),
            body,
        ))
    }
}
