//! Incident Cockpit, mirroring `IncidentSurface.svelte`, `IncidentView.svelte`
//! and the filters. The queue lives in `incident_queue.rs` and guided
//! investigation in `incident_guide.rs`.

use super::{Inspector, surface};
use crate::backend::Scope;
use crate::incidents::{self, Empty, Filter, guidance::Enrichment};
use crate::theme::{Palette, Tokens};
use crate::ui::{self, Tone};
use gpui_kit::TestSupportExt;
use gpui_kit::{AnyElement, Context, SharedString, Task, Window, div, prelude::*, px};
use kubecove_lib::models::{
    IncidentCockpitItem as Item, IncidentCockpitSummary, IncidentSeverity as Severity,
    ResourceDetailsFull, ResourceTopology,
};
use std::rc::Rc;

pub(super) struct IncidentsState {
    /// `None` while the first read is pending.
    pub(super) load: Option<Result<IncidentCockpitSummary, String>>,
    fetching: bool,
    refresh_error: Option<String>,
    context: String,
    pub(super) filter: Filter,
    selected: Option<String>,
    enriched: Option<String>,
    pub(super) details: (Enrichment, Option<ResourceDetailsFull>),
    pub(super) topology: (Enrichment, Option<ResourceTopology>),
    pub(super) copied: Option<(String, bool)>,
    pub(super) show_step_context: bool,
    request: Rc<()>,
    tasks: [Option<Task<()>>; 3],
}

impl IncidentsState {
    pub(super) fn new() -> Self {
        Self {
            load: None,
            fetching: false,
            refresh_error: None,
            context: String::new(),
            filter: Filter::All,
            selected: None,
            enriched: None,
            details: (Enrichment::Idle, None),
            topology: (Enrichment::Idle, None),
            copied: None,
            show_step_context: false,
            request: Rc::new(()),
            tasks: [None, None, None],
        }
    }

    pub(super) fn items(&self) -> &[Item] {
        match &self.load {
            Some(Ok(summary)) => &summary.items,
            _ => &[],
        }
    }

    pub(super) fn selected_key(&self) -> Option<&str> {
        self.selected.as_deref()
    }
}

pub(super) fn severity_badge(item: &Item) -> gpui_kit::Div {
    let label = incidents::severity_label(item);
    if item.severity == Severity::Degraded {
        ui::status_badge(Tone::Error, label)
    } else {
        ui::outline_badge(label)
    }
}

impl Inspector {
    pub(super) fn open_incident_filter(&mut self, filter: Filter, cx: &mut Context<Self>) {
        self.open_page(super::Page::Incidents, cx);
        if self.page == super::Page::Incidents {
            self.incidents.filter = filter;
            self.incidents.selected = None;
            self.incidents.enriched = None;
            self.enrich_incident(cx);
            cx.notify();
        }
    }

    /// Reads the cockpit for the current context; a context change discards
    /// the previous selection and any stale reply.
    pub(super) fn load_incidents(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.incidents;
        if self.scope.context.is_empty() {
            return;
        }
        if state.context != self.scope.context {
            *state = IncidentsState::new();
            state.context = self.scope.context.clone();
        }
        state.request = Rc::new(());
        state.fetching = true;
        state.refresh_error = None;
        state.enriched = None;
        let request = state.request.clone();
        let context = state.context.clone();
        let backend = self.backend.clone();
        let read_context = context.clone();
        let read = crate::read_retry::read(cx.background_executor().clone(), move || {
            backend.incidents(read_context.clone())
        });
        state.tasks[0] = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.incidents;
                if !Rc::ptr_eq(&state.request, &request) {
                    return;
                }
                state.fetching = false;
                // `loadIncidents` prefixes each warning with its cluster.
                let result = result
                    .map(|mut summary| {
                        for warning in &mut summary.warnings {
                            *warning = format!("{context}: {warning}");
                        }
                        summary
                    })
                    .map_err(|error| error.message);
                let success = result.is_ok();
                if let Err(error) = &result
                    && matches!(state.load, Some(Ok(_)))
                {
                    state.refresh_error = Some(error.clone());
                } else {
                    state.load = Some(result);
                }
                this.enrich_incident(cx);
                if success {
                    this.page_read_finished(super::Page::Incidents, cx);
                } else {
                    this.invalidate_page_read(super::Page::Incidents);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn cancel_incident_reads(&mut self) {
        self.incidents.tasks = [None, None, None];
        self.incidents.request = Rc::new(());
        self.incidents.fetching = false;
    }

    /// Loads details and, for Pods, the namespace's ownership graph for the
    /// selected incident, like the web's detail and topology queries.
    fn enrich_incident(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.incidents;
        let surface =
            incidents::surface_state(state.items(), state.filter, state.selected.as_deref());
        let Some(item) = surface.selected else {
            state.tasks[1] = None;
            state.tasks[2] = None;
            state.enriched = None;
            state.details = (Enrichment::Idle, None);
            state.topology = (Enrichment::Idle, None);
            return;
        };
        let key = incidents::item_key(item);
        if state.enriched.as_deref() == Some(key.as_str()) {
            return;
        }
        let resource = item.resource.clone();
        state.selected = Some(key.clone());
        state.enriched = Some(key);
        let request = state.request.clone();
        state.details = (Enrichment::Loading, None);
        let read = self.backend.details(resource.clone());
        let selected = state.enriched.clone();
        state.tasks[1] = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.incidents;
                if Rc::ptr_eq(&state.request, &request) && state.enriched == selected {
                    state.details = match result {
                        Ok(details) => (Enrichment::Ready, Some(details)),
                        Err(_) => (Enrichment::Error, None),
                    };
                    cx.notify();
                }
            });
        }));
        let Some(namespace) = resource
            .namespace
            .clone()
            .filter(|_| resource.kind == "Pod")
        else {
            state.topology = (Enrichment::Idle, None);
            state.tasks[2] = None;
            return;
        };
        state.topology = (Enrichment::Loading, None);
        let request = state.request.clone();
        let backend = self.backend.clone();
        let scope = Scope {
            context: resource.cluster.clone(),
            namespace: Some(namespace),
        };
        let selected = state.enriched.clone();
        state.tasks[2] = Some(cx.spawn(async move |this, cx| {
            let result = crate::read_retry::cancelled_read(
                || backend.topology(scope.clone()),
                |delay| cx.background_executor().timer(delay),
            )
            .await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.incidents;
                if Rc::ptr_eq(&state.request, &request) && state.enriched == selected {
                    state.topology = match result {
                        Ok(topology) => (Enrichment::Ready, Some(topology)),
                        Err(_) => (Enrichment::Error, None),
                    };
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn select_incident(&mut self, key: String, cx: &mut Context<Self>) {
        self.incidents.selected = Some(key);
        self.enrich_incident(cx);
        cx.notify();
    }

    fn set_incident_filter(&mut self, filter: Filter, cx: &mut Context<Self>) {
        self.incidents.filter = filter;
        self.enrich_incident(cx);
        cx.notify();
    }

    fn overview_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let counts = incidents::counts(self.incidents.items());
        let metrics = [
            ("All", counts.total, Tokens::foreground()),
            ("Degraded", counts.degraded, Tokens::destructive()),
            ("Needs attention", counts.attention, Palette::amber400()),
            ("Restart evidence", counts.restarted, Palette::sky400()),
            ("Warnings", counts.warning, Tokens::muted_foreground()),
        ]
        .map(|(label, value, tone)| {
            div()
                .flex_1()
                .min_w_0()
                .bg(Tokens::background().opacity(0.8))
                .px(px(12.))
                .py(px(8.))
                .child(ui::caps(label, 10.).line_height(px(15.)).truncate())
                .child(
                    ui::semibold(ui::text(value.to_string(), 16., 24.))
                        .mt(px(2.))
                        .text_color(tone),
                )
        });
        let fetching = self.incidents.fetching;
        // The incident scope is the workspace (every namespace), not the
        // Resources page's namespace selection.
        let scanning = "Scanning all namespaces";
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            // `grid-cols-[minmax(12rem,1.5fr)_repeat(5,minmax(4.5rem,0.55fr))_auto]`:
            // the metrics span five 0.55fr columns and their four 12px gaps.
            .child(
                div()
                    .flex_basis(px(0.))
                    .flex_grow(1.5)
                    .min_w(px(192.))
                    .child(ui::semibold(ui::sm(self.workspace_name.clone())).truncate())
                    .child(ui::muted(ui::xs(scanning)).mt(px(4.)).truncate()),
            )
            .child(
                div()
                    .flex()
                    .flex_basis(px(48.))
                    .flex_grow(2.75)
                    .gap(px(1.))
                    .overflow_hidden()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::border())
                    .children(metrics),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(8.))
                    .child(
                        surface::small_button(
                            Some(if fetching {
                                "loader-circle"
                            } else {
                                "rotate-ccw"
                            }),
                            "Refresh",
                        )
                        .id("incidents-refresh")
                        .test_support()
                        .when(fetching, surface::disabled)
                        .when(!fetching, |button| {
                            button
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| this.load_incidents(cx)))
                        }),
                    )
                    .child(
                        surface::small_button(Some("external-link"), "Open resources")
                            .id("incidents-open-resources")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_incident_resources(this.incidents.filter, window, cx)
                            })),
                    ),
            )
    }

    fn filter_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let counts = incidents::counts(self.incidents.items());
        div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .children(
                incidents::filter_options(counts).map(|(filter, label, count)| {
                    let active = self.incidents.filter == filter;
                    let button = surface::small_button(None, label);
                    let button = if active {
                        button
                            .border_color(gpui_kit::transparent_black())
                            .bg(Tokens::primary())
                            .text_color(Tokens::primary_foreground())
                    } else {
                        button
                    };
                    button
                        .id(SharedString::from(format!("incident-filter:{label}")))
                        .test_support()
                        .cursor_pointer()
                        .child(
                            surface::secondary_badge(count.to_string())
                                .ml(px(4.))
                                .px(px(6.))
                                .rounded(px(4.)),
                        )
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.set_incident_filter(filter, cx)),
                        )
                }),
            )
    }

    pub(super) fn incidents_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let body: AnyElement = match &self.incidents.load {
            None => surface::loading(),
            Some(Err(error)) => self.page_failure("Incidents unavailable", error.clone(), cx),
            Some(Ok(summary)) => {
                let warnings = summary.warnings.join(" ");
                let state = &self.incidents;
                let empty = incidents::surface_state(
                    &summary.items,
                    state.filter,
                    state.selected.as_deref(),
                )
                .empty;
                // `@5xl:h-[calc(100dvh-20rem)] @5xl:min-h-[32rem]`.
                let height = (window.viewport_size().height - px(320.)).max(px(512.));
                let content = match empty {
                    Empty::Clean => self.empty_state(
                        "No active incident signals",
                        "Scope looks clean.",
                        cx,
                        true,
                    ),
                    Empty::Filtered => self.empty_state(
                        "No matching incident signals",
                        "Change severity filter to see other signals.",
                        cx,
                        false,
                    ),
                    Empty::Ready => div()
                        .flex()
                        .gap(px(12.))
                        .h(height)
                        // `grid-cols-[minmax(18rem,0.42fr)_minmax(0,0.58fr)]`.
                        .child(
                            div()
                                .flex_basis(px(0.))
                                .flex_grow(0.42)
                                .min_w(px(288.))
                                .h_full()
                                .child(self.queue(cx)),
                        )
                        .child(
                            div()
                                .flex_basis(px(0.))
                                .flex_grow(0.58)
                                .min_w_0()
                                .h_full()
                                .child(self.incident_guide(cx)),
                        )
                        .into_any_element(),
                };
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap(px(12.))
                    .child(self.overview_section(cx))
                    .when_some(self.incidents.refresh_error.clone(), |body, error| {
                        body.child(self.page_failure("Incident refresh failed", error, cx))
                    })
                    .when(!warnings.is_empty(), |body| {
                        body.child(
                            ui::panel()
                                .id("incident-warnings")
                                .test_support()
                                .aria_label("Partial incident data")
                                .aria_description(warnings.clone())
                                .px(px(12.))
                                .py(px(8.))
                                .border_color(Palette::amber500(0.4))
                                .child(ui::semibold(ui::xs("Partial incident data")))
                                .child(ui::muted(ui::xs(warnings))),
                        )
                    })
                    .child(self.filter_buttons(cx))
                    .child(content)
                    .into_any_element()
            }
        };
        surface::frame("triangle-alert", "Incident Cockpit", true, body)
    }
}
