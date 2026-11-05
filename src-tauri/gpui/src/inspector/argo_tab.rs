//! The Argo CD tab, mirroring `ArgoApplicationDetails.svelte` over the
//! Kubernetes transport. Refresh, Sync and the connection controls render
//! disabled: connected Argo CD and Application operations (ADR 0013, ADR 0004)
//! are not ported. Read-only YAML shows as plain text instead of an editor.

use super::argo_panels::{delivery, navigator, panel};
use super::argo_snapshot::{DiffView, snapshot};
use super::controls::{self, Variant};
use super::delivery_ui as dui;
use super::detail::{Detail, ResourceDetail};
use super::{Inspector, surface};
use crate::argo_app::ArgoView;
use crate::model::health_source;
use crate::theme::{MONO, Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, Div, SharedString, Task, Window, div, prelude::*, px};
use kubecove_lib::models::ArgoApplicationRef;

enum ArgoLoad {
    Loading,
    Ready(Box<ArgoView>),
    Failed(String),
}

pub(super) struct ArgoTab {
    load: ArgoLoad,
    /// The change navigator's focused resource, by index into `changes`.
    pub(super) focus: Option<usize>,
    pub(super) view: DiffView,
    /// Removals whose live YAML is shown, and diffs shown in full.
    pub(super) expanded: Vec<usize>,
    pub(super) full: Vec<usize>,
    evidence: bool,
    _task: Option<Task<()>>,
}

impl Inspector {
    pub(super) fn argo_tab_state(&mut self) -> Option<&mut ArgoTab> {
        match &mut self.detail {
            Detail::Ready(detail) => detail.argo.as_mut(),
            _ => None,
        }
    }

    /// Reads the inspector snapshot the first time the tab opens.
    pub(super) fn open_argo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        if detail.argo.is_some() {
            return;
        }
        let summary = detail.data.summary.clone();
        let read = self.backend.argo_inspector(ArgoApplicationRef {
            name: summary.name.clone(),
            namespace: summary.namespace.clone(),
            project: None,
            resource_version: None,
            uid: None,
            api_version: summary.api_version.clone(),
            context: Some(summary.cluster.clone()),
            workspace_id: None,
        });
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let Detail::Ready(detail) = &mut this.detail else {
                    return;
                };
                let view = result.map(|inspector| {
                    Box::new(ArgoView::new(
                        &inspector,
                        &detail.data.summary,
                        &detail.data.yaml,
                    ))
                });
                if let Some(state) = detail.argo.as_mut() {
                    state.load = match view {
                        Ok(view) => ArgoLoad::Ready(view),
                        Err(error) => ArgoLoad::Failed(error.message),
                    };
                }
                cx.notify();
            });
        });
        detail.argo = Some(ArgoTab {
            load: ArgoLoad::Loading,
            focus: None,
            view: DiffView::Changes,
            expanded: Vec::new(),
            full: Vec::new(),
            evidence: false,
            _task: Some(task),
        });
    }
}

/// `SignalCard`: `bg-surface-1 p-2.5`, count and label on one baseline over
/// a `text-[0.65rem]` description.
fn signal(value: usize, label: &str, description: &str) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("argo-signal:{label}")))
        .test_support()
        .aria_label(value.to_string())
        .flex_1()
        .flex_basis(px(0.))
        .min_w_0()
        .p(px(10.))
        .bg(Tokens::card())
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(6.))
                .child(ui::semibold(ui::text(value.to_string(), 16., 24.)))
                // `items-baseline` aligns the label's first line with the
                // count; GPUI aligns wrapped text by its last line.
                .child(ui::medium(ui::xs(label.to_owned())).min_w_0().mt(px(5.))),
        )
        .child(ui::muted(ui::text(description.to_owned(), 10.4, 14.)).mt(px(2.)))
}

/// `raw()` in `HealthAssessmentBadge.svelte`: strings as-is, other values as JSON.
fn raw(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

pub(super) fn argo_tab(
    detail: &ResourceDetail,
    timezone: crate::preferences::TimestampTimezone,
    cx: &mut Context<Inspector>,
) -> AnyElement {
    let Some(state) = detail.argo.as_ref() else {
        return div().into_any_element();
    };
    let summary = &detail.data.summary;
    let view = match &state.load {
        ArgoLoad::Ready(view) => Some(view.as_ref()),
        _ => None,
    };
    let loading = matches!(state.load, ArgoLoad::Loading);
    let assessment = &detail.opened_health;
    let health = view.map_or_else(
        || summary.status.clone().unwrap_or_else(|| "Unknown".into()),
        |view| view.health_status.clone(),
    );
    let sync = view.map_or_else(|| "Unknown".to_owned(), |view| view.sync_status.clone());
    let source = view.map_or("Kubernetes Application status".to_owned(), |view| {
        view.source.clone()
    });
    let scope_rows = [
        "Operation scope: Argo CD Application".to_owned(),
        "Transport: Kubernetes API".to_owned(),
        format!("Context: {}", summary.cluster),
        format!(
            "Namespace: {}",
            summary.namespace.as_deref().unwrap_or("cluster-scoped")
        ),
        format!("Kind: {}", summary.kind),
        format!("Resource: {}", summary.name),
    ];
    // `flex flex-wrap justify-between gap-3`: the scope card's long
    // description makes the left column wider than any pane, so the
    // operation buttons always wrap onto their own line.
    let header = div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.))
        .child(
            div()
                .w_full()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::semibold(ui::heading(summary.name.clone(), 14., 20.)).truncate())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .h(px(20.))
                                .px(px(8.))
                                .child(ui::icon("circle-dot", 10., Tokens::primary()))
                                .child(ui::medium(ui::text(source, 10., 14.))),
                        ),
                )
                .child(div().mt(px(8.)).child(surface::health_badge(assessment)))
                .child(
                    div()
                        .id("argo-evidence")
                        .mt(px(8.))
                        .p(px(8.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Tokens::border())
                        .bg(Tokens::background().opacity(0.4))
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(state) = this.argo_tab_state() {
                                state.evidence = !state.evidence;
                                cx.notify();
                            }
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(ui::icon(
                                    if state.evidence { "chevron-down" } else { "chevron-right" },
                                    12.,
                                    Tokens::foreground(),
                                ))
                                .child(ui::medium(ui::xs("Health evidence"))),
                        )
                        // `<ul class="mt-2 grid gap-1.5">`: a medium source label, then
                        // the raw value and reason (the web's em dash is UI text).
                        .when(state.evidence, |box_| {
                            box_.child(div().mt(px(8.)).flex().flex_col().gap(px(6.)).children(
                                assessment.evidence.iter().map(|evidence| {
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .child(ui::medium(ui::xs(format!(
                                            "{}:",
                                            health_source(std::slice::from_ref(&evidence.source))
                                        ))))
                                        .child(ui::muted(ui::xs(format!(
                                            " {} \u{2014} {}{}",
                                            raw(&evidence.raw),
                                            evidence.reason,
                                            if evidence.current { "" } else { " (historical)" }
                                        ))))
                                }),
                            ))
                        }),
                )
                .child(
                    div()
                        .id("argo-app-status")
                        .test_support()
                        .aria_label(format!("{} {health} {sync}", summary.name))
                        .mt(px(8.))
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::muted(ui::xs("Raw Argo:")))
                        .child(ui::outline_badge(format!("Health {health}")))
                        .child(ui::outline_badge(format!("Sync {sync}")))
                        .when_some(view.and_then(|view| view.operation_state.clone()), |row, state| {
                            row.child(surface::secondary_badge(state))
                        }),
                )
                .child(
                    ui::muted(ui::text(
                        if loading {
                            "Loading Application and managed resources…".to_owned()
                        } else {
                            format!(
                                "Revision {}",
                                view.map_or("Revision unavailable", |view| &view.resolved_revision)
                            )
                        },
                        12.,
                        19.5,
                    ))
                    .mt(px(4.)),
                )
                .child(
                    div()
                        .mt(px(12.))
                        .p(px(12.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Tokens::border())
                        .bg(Tokens::background().opacity(0.5))
                        .font_family(MONO)
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .children(scope_rows.chunks(2).map(|pair| {
                            div().flex().gap(px(4.)).children(
                                pair.iter().map(|row| div().flex_1().flex_basis(px(0.)).min_w_0().child(row.clone())),
                            )
                        }))
                        .child(
                            ui::muted(ui::xs(
                                "Unavailable in native client. These operations target the Argo CD Application, not a selected managed Kubernetes resource.",
                            ))
                            .font_family(crate::theme::SANS),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .child(split(Variant::Outline, "refresh-cw", "Refresh"))
                .child(split(Variant::Default, "rotate-ccw", "Sync")),
        );
    let counts = view.map(|view| view.counts).unwrap_or_default();
    let signals = div()
        .mt(px(12.))
        .flex()
        .gap(px(1.))
        .overflow_hidden()
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::border())
        .child(signal(
            counts.needs_sync,
            "Out of sync",
            "Needs reconciliation",
        ))
        .child(signal(
            counts.needs_attention,
            "Needs attention",
            "Health is not settled",
        ))
        .child(signal(counts.prune, "Prune", "No longer in target"));
    let top = div()
        .overflow_hidden()
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .shadow_sm()
        .child(div().p(px(12.)).child(header).child(signals));
    let connection = panel()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
        .child(surface::disabled(
            div()
                .flex()
                .items_center()
                .justify_between()
                .w(px(288.))
                .h(px(28.))
                .px(px(10.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::input())
                .child(ui::xs("Automatic · Kubernetes"))
                .child(ui::icon("chevron-down", 14., Tokens::muted_foreground())),
        ))
        .child(
            ui::muted(ui::xs(
                "Argo RBAC is not evaluated. Exact manifests require a connected profile.",
            ))
            .flex_1()
            .min_w_0(),
        )
        .child(surface::disabled(dui::sm_button(
            Variant::Outline,
            "Connect Argo CD",
        )));
    let mut column = div()
        .id("argo-tab")
        .flex()
        .flex_col()
        .gap(px(12.))
        .min_w_0()
        .child(top)
        .child(connection);
    if let ArgoLoad::Failed(error) = &state.load {
        column = column.child(surface::failed(
            "Kubernetes Application snapshot unavailable",
            error.clone(),
        ));
    }
    if let Some(view) = view {
        column = column
            .children(view.conditions.iter().map(|(kind, message)| {
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .p(px(12.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(Palette::amber500(0.25))
                    .bg(Palette::amber500(0.05))
                    .child(ui::icon("triangle-alert", 14., Palette::amber400()).mt(px(2.)))
                    .child(
                        div()
                            .min_w_0()
                            .child(ui::medium(ui::xs(kind.clone())))
                            .child(ui::muted(ui::text(message.clone(), 12., 19.5)).mt(px(2.))),
                    )
            }))
            .child(navigator(view, state, cx))
            .child(delivery(view, timezone));
    }
    let failed = matches!(state.load, ArgoLoad::Failed(_));
    column
        .child(snapshot(view, state, failed, cx))
        .into_any_element()
}

/// The Refresh and Sync split buttons, disabled.
fn split(variant: Variant, icon: &str, label: &str) -> Div {
    let color = if variant == Variant::Default {
        Tokens::primary_foreground()
    } else {
        Tokens::foreground()
    };
    surface::disabled(
        div()
            .flex()
            .child(
                controls::button(variant, 24., false)
                    .rounded_r_none()
                    .child(ui::icon(icon, 12., color))
                    .child(label.to_owned()),
            )
            .child(
                controls::button(variant, 24., false)
                    .w(px(24.))
                    .px_0()
                    .rounded_l_none()
                    .when(variant == Variant::Outline, |button| button.border_l_0())
                    .child(ui::icon("chevron-down", 12., color)),
            ),
    )
}
