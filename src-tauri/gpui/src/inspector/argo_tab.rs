//! The Argo CD inspector uses the selected transport and canonical Application.
//! Connection credentials and operation reviews stay behind the Rust backend.

use super::argo_panels::{delivery, navigator};
use super::argo_snapshot::{DiffView, snapshot};
use super::controls::{self, Variant};
use super::detail::{Detail, ResourceDetail};
use super::feedback::Disclosure;
use super::{Inspector, surface};
use crate::argo_app::ArgoView;
use crate::model::health_source;
use crate::targets::TestSupportExt;
use crate::theme::{MONO, Palette, Tokens};
use crate::ui;
use gpui_kit::{AnyElement, Context, Div, SharedString, Task, Window, div, prelude::*, px};
use kubecove_lib::models::{AppError, ArgoApplicationInspector, ArgoApplicationRef};
use std::rc::Rc;

#[path = "argo_comparison.rs"]
mod comparison;

enum ArgoLoad {
    Loading,
    Ready(Box<ArgoView>),
    Failed(AppError),
}

pub(super) struct ArgoTab {
    load: ArgoLoad,
    /// The change navigator's focused resource, by index into `changes`.
    pub(super) focus: Option<usize>,
    pub(super) view: DiffView,
    /// Removals whose live YAML is shown, and diffs shown in full.
    pub(super) expanded: Vec<usize>,
    pub(super) full: Vec<usize>,
    pub(super) highlights: Vec<Vec<super::argo_highlight::Highlights>>,
    evidence: bool,
    pub(super) inspector: Option<ArgoApplicationInspector>,
    pub(super) operation: super::argo_operations::Phase,
    pub(super) options: super::argo_options::Options,
    pub(super) editors: Vec<(
        gpui_kit::Entity<gpui_kit::component::input::EditorState>,
        gpui_kit::Entity<gpui_kit::component::input::EditorState>,
    )>,
    request: Rc<()>,
    _task: Option<Task<()>>,
    comparison: Option<Task<()>>,
    comparison_request: Rc<()>,
    pub(super) comparison_error: Option<String>,
    pub(super) refresh_options: bool,
}

impl ArgoTab {
    pub(super) fn configured_revision(&self) -> &str {
        match &self.load {
            ArgoLoad::Ready(view) => &view.configured_revision,
            ArgoLoad::Loading | ArgoLoad::Failed(_) => "Revision unavailable",
        }
    }
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
        let connection_id = self.argo_connection_id();
        let workspace_id = self.argo_workspace_id();
        let source = self.argo_source_key().map(str::to_owned);
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        if detail.argo.is_some() {
            return;
        }
        let summary = detail.data.summary.clone();
        let read = self.backend.inspect_argo(crate::backend::ArgoInspection {
            connection_id,
            source,
            application: ArgoApplicationRef {
                name: summary.name.clone(),
                namespace: summary.namespace.clone(),
                project: None,
                resource_version: None,
                uid: None,
                api_version: summary.api_version.clone(),
                context: Some(summary.cluster.clone()),
                workspace_id: Some(workspace_id),
            },
        });
        let request = Rc::new(());
        let current = request.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Detail::Ready(detail) = &mut this.detail else {
                    return;
                };
                if detail
                    .argo
                    .as_ref()
                    .is_none_or(|state| !Rc::ptr_eq(&current, &state.request))
                {
                    return;
                }
                let view = result.map(|inspector| {
                    let _span = crate::frame_profile::scope("argo", "prepare_view");
                    let view = Box::new(ArgoView::new(
                        &inspector,
                        &detail.data.summary,
                        &detail.data.yaml,
                    ));
                    (view, inspector)
                });
                if let Some(state) = detail.argo.as_mut() {
                    state.load = match view {
                        Ok((view, inspector)) => {
                            let highlights_span = crate::frame_profile::scope("argo", "highlights");
                            state.highlights = view
                                .changes
                                .iter()
                                .map(|change| {
                                    super::argo_highlight::lines(
                                        &mut this.argo_highlighter,
                                        change.diff.as_deref().unwrap_or_default(),
                                        &gpui_kit::component::Theme::global(cx).highlight_theme,
                                    )
                                })
                                .collect();
                            drop(highlights_span);
                            let editors_span = crate::frame_profile::scope("argo", "editors");
                            use gpui_kit::{AppContext, component::input::EditorState};
                            state.editors = view
                                .changes
                                .iter()
                                .map(|change| {
                                    let desired = cx.new(|cx| {
                                        EditorState::new(window, cx)
                                            .language("yaml")
                                            .line_number(true)
                                            .default_value(change.desired.clone())
                                    });
                                    let live = cx.new(|cx| {
                                        EditorState::new(window, cx)
                                            .language("yaml")
                                            .line_number(true)
                                            .default_value(change.live.clone())
                                    });
                                    (desired, live)
                                })
                                .collect();
                            drop(editors_span);
                            state.inspector = Some(inspector);
                            let options: Vec<super::argo_options::HistoryChoice> = state
                                .inspector
                                .as_ref()
                                .unwrap()
                                .history
                                .iter()
                                .enumerate()
                                .map(|(index, h)| super::argo_options::HistoryChoice {
                                    index,
                                    revision: h
                                        .revision
                                        .clone()
                                        .unwrap_or_else(|| "Unknown revision".into())
                                        .into(),
                                    id: h.id,
                                })
                                .collect();
                            state.options.picker.update(cx, |picker, cx| {
                                picker.set_items(
                                    gpui_kit::component::select::SearchableVec::new(
                                        options.clone(),
                                    ),
                                    window,
                                    cx,
                                );
                                if let Some(first) = options.first() {
                                    picker.set_selected_value(&first.index, window, cx);
                                }
                            });
                            state.options.prune = Some(
                                serde_yaml::from_str::<serde_json::Value>(&detail.data.yaml)
                                    .ok()
                                    .and_then(|v| {
                                        v.pointer("/spec/syncPolicy/automated/prune")
                                            .and_then(|v| v.as_bool())
                                    })
                                    .unwrap_or(false),
                            );
                            state.options.default_prune = state.options.prune.unwrap_or(false);
                            state.options.revision = cx.new(|cx| {
                                gpui_kit::component::input::InputState::new(window, cx)
                                    .placeholder(view.configured_revision.clone())
                            });
                            ArgoLoad::Ready(view)
                        }
                        Err(error) => ArgoLoad::Failed(error),
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
            highlights: Vec::new(),
            evidence: false,
            inspector: None,
            operation: Default::default(),
            options: super::argo_options::Options::new(window, cx),
            editors: Vec::new(),
            request,
            _task: Some(task),
            comparison: None,
            comparison_request: Rc::new(()),
            comparison_error: None,
            refresh_options: false,
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
pub(super) fn raw(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The selected Argo CD connection as the Application operations see it.
pub(super) struct ArgoConnectionState {
    pub connected: bool,
    /// The reason the web's `argoOperationAvailability` gives when operations are unavailable.
    pub unavailable: Option<String>,
    /// An operation on this Application is still running.
    pub busy: bool,
}

pub(super) fn argo_tab(
    detail: &ResourceDetail,
    timezone: crate::preferences::TimestampTimezone,
    connection: Div,
    selected: ArgoConnectionState,
    disclosure: &Disclosure,
    loading_delays: &super::loading::LoadingDelays,
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
    let indicator = loading_delays.phase("argo-detail", loading);
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
        format!(
            "Transport: {}",
            if view.is_some_and(|v| !v.kubernetes) {
                "Connected Argo CD API"
            } else {
                "Kubernetes API"
            }
        ),
        format!("Context: {}", summary.cluster),
        format!(
            "Namespace: {}",
            summary.namespace.as_deref().unwrap_or("cluster-scoped")
        ),
        format!("Kind: {}", summary.kind),
        format!("Resource: {}", summary.name),
    ];
    // The web's OperationScopeCard description. Its reason is the same text the web shows.
    let scope_description = match &selected.unavailable {
        Some(reason) => format!(
            "Unavailable, provider connection: {reason} These operations target the Argo CD Application, not a selected managed Kubernetes resource."
        ),
        None if view.is_some_and(|v| !v.kubernetes) => "Available. Connected Argo CD authorization is checked during operation review. These operations target the Argo CD Application, not a selected managed Kubernetes resource.".to_owned(),
        None => "Available. Kubernetes permission is checked during operation review. These operations target the Argo CD Application, not a selected managed Kubernetes resource.".to_owned(),
    };
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
                                    if state.evidence {
                                        "chevron-down"
                                    } else {
                                        "chevron-right"
                                    },
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
                                            if evidence.current {
                                                ""
                                            } else {
                                                " (historical)"
                                            }
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
                        .when_some(
                            view.and_then(|view| view.operation_state.clone()),
                            |row, state| row.child(surface::secondary_badge(state)),
                        ),
                )
                .child(
                    ui::muted(ui::text(
                        if indicator.shown() {
                            "Loading Application and managed resources…".to_owned()
                        } else if !indicator.ready() {
                            String::new()
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
                            div().flex().gap(px(4.)).children(pair.iter().map(|row| {
                                div()
                                    .flex_1()
                                    .flex_basis(px(0.))
                                    .min_w_0()
                                    .child(row.clone())
                            }))
                        }))
                        .child(
                            ui::muted(ui::xs(scope_description.clone()))
                                .font_family(crate::theme::SANS)
                                .id("argo-operation-scope")
                                .test_support()
                                .aria_label(scope_description),
                        ),
                ),
        )
        .child(super::argo_options::controls(
            state,
            &summary.name,
            loading || selected.busy,
            selected.unavailable.is_some(),
            cx,
        ));
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
    let mut column = div()
        .id("argo-tab")
        .test_support()
        .aria_label(view.map_or_else(String::new, |v| v.source.clone()))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(12.))
        .min_w_0()
        .child(top)
        .when(
            !matches!(state.operation, super::argo_operations::Phase::Idle),
            |column| column.child(super::argo_operations::panel(&state.operation, cx)),
        )
        .child(connection);
    if let ArgoLoad::Failed(error) = &state.load {
        column = column
            .child(Inspector::argo_snapshot_failure(
                error,
                selected.connected,
                disclosure,
                cx,
            ))
            .child(
                controls::button(Variant::Outline, 24., false)
                    .id("argo-snapshot-retry")
                    .test_support()
                    .child("Retry snapshot")
                    .on_click(cx.listener(|this, _, window, cx| this.reload_argo(window, cx))),
            );
    }
    if let Some(fallback) = state
        .inspector
        .as_ref()
        .and_then(|s| s.connected_fallback.as_ref())
    {
        column = column.child(surface::failed(
            "Connected Argo CD unavailable. Showing Kubernetes fallback.",
            fallback.failure.message.clone(),
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
            .child(delivery(view, state, timezone));
    }
    if let Some(error) = &state.comparison_error {
        column = column
            .child(surface::failed(
                "Resource comparison unavailable",
                error.clone(),
            ))
            .child(
                controls::button(Variant::Outline, 24., false)
                    .id("argo-comparison-retry")
                    .test_support()
                    .child("Retry comparison")
                    .on_click(cx.listener(|this, _, window, cx| {
                        let index = this.argo_tab_state().and_then(|s| s.focus);
                        this.focus_argo_resource(index, window, cx);
                    })),
            );
    }
    let failed = matches!(state.load, ArgoLoad::Failed(_));
    column
        .child(snapshot(view, state, failed, indicator, cx))
        .into_any_element()
}
