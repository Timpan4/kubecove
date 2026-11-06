//! Guided investigation pane, mirroring `IncidentGuide.svelte`. Steps open the
//! resource in the detail pane. Guarded actions hand off to the Actions tab.

use super::{Inspector, Page, detail::DetailTab, incidents::severity_badge, surface};
use crate::incidents::{
    self,
    guidance::{self, Enrichment, Evidence, Tone},
    resource_label,
};
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, Div, Hsla, SharedString, div, prelude::*, px};
use kubecove_lib::models::ResourceSummary;

fn evidence_color(tone: Tone) -> Hsla {
    match tone {
        Tone::Error => Tokens::destructive(),
        Tone::Warning => Palette::amber500(1.),
        Tone::Info => Palette::sky500(1.),
        Tone::Neutral => Tokens::muted_foreground(),
    }
}

/// `TimestampText precision="second"` in local time.
pub(super) fn timestamp(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value).map_or_else(
        |_| value.to_owned(),
        |time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        },
    )
}

/// `rounded-md border bg-background/35` sections.
fn section() -> Div {
    div()
        .rounded(px(6.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.35))
}

fn label(text: &str) -> Div {
    ui::caps(text, 12.).line_height(px(16.))
}

fn relaxed(text: impl Into<SharedString>) -> Div {
    ui::muted(ui::text(text, 12., 19.5))
}

fn evidence_row(evidence: &Evidence) -> Div {
    div()
        .relative()
        .overflow_hidden()
        .rounded(px(6.))
        .border_t_1()
        .border_r_1()
        .border_b_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .pl(px(16.))
        .pr(px(12.))
        .py(px(8.))
        .child(surface::left_rule(evidence_color(evidence.tone)).rounded_l(px(6.)))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_start()
                .justify_between()
                .gap(px(8.))
                .child(ui::semibold(ui::xs(evidence.label.clone())))
                .when_some(evidence.timestamp.as_deref(), |row, value| {
                    row.child(ui::muted(ui::text(timestamp(value), 10., 16.)))
                }),
        )
        .child(relaxed(evidence.detail.clone()).mt(px(4.)))
}

impl Inspector {
    /// `onResourceSelect` / `onResourceInspect`: show the resource in the
    /// Resources page's detail pane.
    fn inspect_resource(
        &mut self,
        resource: ResourceSummary,
        tab: Option<DetailTab>,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        self.select_namespace(None, window, cx);
        if self.page == Page::Resources {
            self.open_resource_on(resource, tab, window, cx);
        }
    }

    fn step_button(
        &self,
        index: usize,
        title: String,
        target: ResourceSummary,
        tab: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = match tab {
            "details" => DetailTab::Details,
            "events" => DetailTab::Events,
            "logs" => DetailTab::Logs,
            "yaml" => DetailTab::Yaml,
            other => unreachable!("unknown incident detail tab {other}"),
        };
        ui::outline_button()
            .id(SharedString::from(format!("incident-step:{index}")))
            .test_support()
            .aria_label(title.clone())
            .h(px(24.))
            .pl(px(8.))
            .pr(px(6.))
            .cursor_pointer()
            .child(ui::medium(ui::text(title, 12., 19.5)))
            .child(ui::icon("arrow-right", 12., Tokens::foreground()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.inspect_resource(target.clone(), Some(tab), window, cx)
            }))
            .into_any_element()
    }

    fn steps_section(&self, steps: Vec<guidance::Step>, cx: &mut Context<Self>) -> Div {
        let context = self.incidents.show_step_context;
        let context_help = "Open evidence in the detail pane without losing queue selection.";
        let steps: Vec<AnyElement> = steps
            .into_iter()
            .enumerate()
            .map(|(index, step)| {
                if !context {
                    return self.step_button(index, step.title, step.target, step.tab, cx);
                }
                let tab = if step.tab == "details" {
                    "details"
                } else {
                    step.tab
                };
                div()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .gap(px(12.))
                    .min_w_0()
                    .p(px(12.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::card())
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(ui::semibold(ui::xs(step.title.clone())))
                                    .child(ui::outline_badge(resource_label(&step.target))),
                            )
                            .child(relaxed(step.description).mt(px(4.))),
                    )
                    .child(div().flex().child(self.step_button(
                        index,
                        format!("Open {tab}"),
                        step.target,
                        step.tab,
                        cx,
                    )))
                    .into_any_element()
            })
            .collect();
        section()
            .p(px(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .justify_between()
                    .gap(px(8.))
                    .mb(px(12.))
                    .child(
                        div()
                            .child(label("Next investigation steps"))
                            .when(context, |title| {
                                title.child(
                                    ui::muted(ui::xs(context_help))
                                        .id("incident-context-help")
                                        .test_support()
                                        .aria_label(context_help)
                                        .mt(px(4.)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .id("incident-step-context")
                            .test_support()
                            .flex()
                            .items_center()
                            .h(px(24.))
                            .px(px(8.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|button| button.bg(Tokens::muted().opacity(0.5)))
                            .child(ui::medium(ui::text(
                                if context {
                                    "Hide context"
                                } else {
                                    "Show context"
                                },
                                12.,
                                19.5,
                            )))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.incidents.show_step_context =
                                    !this.incidents.show_step_context;
                                cx.notify();
                            })),
                    ),
            )
            .child(if context {
                div().flex().flex_wrap().gap(px(8.)).children(
                    steps
                        .into_iter()
                        .map(|step| div().w(gpui_kit::relative(0.49)).child(step)),
                )
            } else {
                div().flex().flex_wrap().gap(px(8.)).children(steps)
            })
    }

    fn actions_section(&self, actions: Vec<guidance::Action>, cx: &mut Context<Self>) -> Div {
        section()
            .p(px(12.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .mb(px(12.))
                    .child(ui::icon("shield-alert", 16., Tokens::muted_foreground()).mt(px(2.)))
                    .child(
                        div().child(label("Available guarded actions")).child(
                            ui::muted(ui::xs(
                                "Choices, not recommendations. Each opens a preview-and-confirm flow; nothing runs here.",
                            ))
                            .mt(px(4.)),
                        ),
                    ),
            )
            .child(div().flex().flex_wrap().gap(px(8.)).children(actions.into_iter().enumerate().map(
                |(index, action)| {
                    let target = &action.target;
                    let scope = format!(
                        "{} · {} · {}/{}",
                        target.cluster,
                        target.namespace.as_deref().unwrap_or("cluster"),
                        target.kind,
                        target.name
                    );
                    let target = action.target.clone();
                    let button = ui::outline_button()
                            .id(SharedString::from(format!("incident-action:{index}")))
                            .test_support()
                            .cursor_pointer()
                            .h(px(24.))
                            .pl(px(8.))
                            .pr(px(6.))
                            .child(ui::medium(ui::text("Review in Actions", 12., 19.5)))
                            .child(ui::icon("arrow-right", 12., Tokens::foreground()))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_resource_on(target.clone(), Some(super::detail::DetailTab::Actions), window, cx);
                            }));
                    div()
                        .w(gpui_kit::relative(0.49))
                        .flex()
                        .flex_col()
                        .justify_between()
                        .gap(px(12.))
                        .p(px(12.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(if action.destructive {
                            Tokens::destructive().opacity(0.35)
                        } else {
                            Tokens::border()
                        })
                        .bg(if action.destructive {
                            Tokens::destructive().opacity(0.05)
                        } else {
                            Tokens::card()
                        })
                        .child(
                            div()
                                .child(ui::semibold(ui::xs(action.label)))
                                .child(relaxed(action.description).mt(px(4.)))
                                .child(
                                    ui::panel()
                                        .mt(px(8.))
                                        .p(px(8.))
                                        .child(ui::muted(ui::xs(scope))),
                                ),
                        )
                        .child(div().flex().child(button))
                },
            )))
    }

    pub(super) fn incident_guide(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.incidents;
        let selected = state.selected_item();
        let container = div()
            .id("incident-guide")
            .test_support()
            .size_full()
            .min_w_0()
            .overflow_y_scroll()
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card());
        let Some(item) = selected else {
            return container.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(4.))
                    .min_h(px(320.))
                    .child(ui::heading("Choose an incident signal", 14., 20.))
                    .child(ui::muted(ui::xs(
                        "Select a queue item to build a guided investigation.",
                    ))),
            );
        };
        let (details_state, details) = (state.details.0, state.details.1.as_ref());
        let topology_state = state.topology.0;
        let owners = guidance::resolve_owner(&item.resource, state.topology.1.as_ref());
        let guide = guidance::build(item, details, &owners, details_state, topology_state);
        let enriching =
            details_state == Enrichment::Loading || topology_state == Enrichment::Loading;
        let failed = details_state == Enrichment::Error || topology_state == Enrichment::Error;
        let resource = item.resource.clone();
        let facts = guide.facts.iter().map(|fact| {
            section()
                .flex_1()
                .min_w_0()
                .p(px(12.))
                .child(label(fact.label))
                .child(ui::semibold(ui::sm(fact.value.clone())).mt(px(4.)))
                .child(relaxed(fact.detail.clone()).mt(px(4.)))
        });
        container
            .child(
                div()
                    .px(px(16.))
                    .py(px(12.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .gap(px(12.))
                            .child(
                                div()
                                    .min_w_0()
                                    .child(label("Guided investigation").mb(px(4.)))
                                    .child(
                                        ui::semibold(ui::text(guide.title.clone(), 16., 22.))
                                            .id("incident-guide-title")
                                            .test_support()
                                            .aria_label(guide.title.clone()),
                                    )
                                    .child(ui::muted(ui::xs(incidents::scope_label(item))).mt(px(4.))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(severity_badge(item))
                                    .child(
                                        surface::small_button(Some("external-link"), "Open in Resources")
                                            .id("incident-open-resource")
                                            .test_support()
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.inspect_resource(resource.clone(), None, window, cx)
                                            })),
                                    ),
                            ),
                    )
                    .child(ui::text(guide.summary.clone(), 14., 22.75).mt(px(12.)).max_w(px(896.)))
                    .child(surface::health_badge(&item.resource.health_assessment).mt(px(12.)))
                    .child(
                        div()
                            .mt(px(8.))
                            .p(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(Tokens::border())
                            .bg(Tokens::background().opacity(0.4))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(ui::text("▶", 8., 16.))
                                    .child(ui::medium(ui::xs("Health evidence"))),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .p(px(16.))
                    .when(enriching || failed, |body| {
                        body.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .px(px(12.))
                                .py(px(8.))
                                .rounded(px(6.))
                                .border_1()
                                .when(failed, |notice| {
                                    notice
                                        .border_color(Palette::amber500(0.4))
                                        .bg(Palette::amber500(0.05))
                                        .text_color(Palette::amber300())
                                })
                                .when(!failed, |notice| {
                                    notice
                                        .border_color(Tokens::border())
                                        .bg(Tokens::background().opacity(0.35))
                                        .text_color(Tokens::muted_foreground())
                                })
                                .when(enriching, |notice| {
                                    notice.child(ui::icon("loader-circle", 16., Tokens::muted_foreground()))
                                })
                                .id("incident-enrichment")
                                .test_support()
                                .aria_label(if failed { "Some live evidence is unavailable. Snapshot guidance remains usable." } else { "Loading live conditions and controller ownership…" })
                                .child(ui::xs(if failed {
                                    "Some live evidence is unavailable. Snapshot guidance remains usable."
                                } else {
                                    "Loading live conditions and controller ownership…"
                                })),
                        )
                    })
                    .child(div().flex().gap(px(8.)).children(facts))
                    .child(
                        section()
                            .child(
                                div()
                                    .px(px(12.))
                                    .py(px(10.))
                                    .border_b_1()
                                    .border_color(Tokens::border())
                                    .child(label("Confirmed evidence")),
                            )
                            .child(if guide.evidence.is_empty() {
                                ui::muted(ui::xs("No detailed evidence is available in this snapshot."))
                                    .p(px(12.))
                            } else {
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .p(px(12.))
                                    .children(guide.evidence.iter().map(evidence_row))
                            }),
                    )
                    .child(
                        div()
                            .p(px(12.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(Palette::amber500(0.3))
                            .bg(Palette::amber500(0.05))
                            .child(ui::caps("Still unknown", 12.).line_height(px(16.)).text_color(Palette::amber300()))
                            .child(div().flex().flex_col().gap(px(6.)).mt(px(8.)).children(
                                guide.missing.iter().map(|missing| {
                                    div()
                                        .flex()
                                        .gap(px(8.))
                                        .child(relaxed("•"))
                                        .child(relaxed(*missing))
                                }),
                            )),
                    )
                    .child(self.steps_section(guide.steps, cx))
                    .when(!guide.actions.is_empty(), |body| {
                        body.child(self.actions_section(guide.actions, cx))
                    }),
            )
    }
}
