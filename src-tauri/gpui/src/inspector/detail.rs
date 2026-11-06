//! Resource detail panel, mirroring `ResourceDetailPanel.svelte`. Each tab
//! lives in its own module; tabs load their data the first time they open.

use super::Inspector;
use super::actions_state::ActionsState;
use super::actions_tab::actions_tab;
use super::argo_tab::{ArgoTab, argo_tab};
use super::details_metadata::MetadataDisclosure;
use super::details_tab::details_tab;
use super::events::EventsState;
use super::events_tab::events_tab;
use super::feedback;
use super::friendly_error;
use super::logs_state::LogsState;
use super::logs_tab::logs_tab;
use super::revisions_tab::{RevisionsState, revisions_tab};
use super::windowed::row_id;
use super::yaml_apply::YamlApply;
use super::yaml_marks::YamlMarks;
use super::yaml_tab::yaml_tab;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::component::input::{EditorState, InputEvent};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, KeyDownEvent, MouseButton, MouseMoveEvent,
    ScrollHandle, Subscription, Window, div, prelude::*, px,
};
use kubecove_lib::models::{AppError, HealthAssessment, ResourceDetailsFull, ResourceSummary};
use std::sync::Arc;

pub(super) enum Detail {
    Closed,
    Loading(Box<ResourceSummary>),
    Failed(Box<FailedDetail>),
    Ready(Box<ResourceDetail>),
}

/// A details read that failed. It keeps the resource so the failure can offer
/// the required permission and return to the same object.
pub(super) struct FailedDetail {
    pub resource: ResourceSummary,
    pub error: AppError,
}

impl Detail {
    /// The resource the panel is open for, which the ownership map selects.
    pub(super) fn resource(&self) -> Option<&ResourceSummary> {
        match self {
            Detail::Loading(resource) => Some(resource),
            Detail::Ready(detail) => Some(&detail.data.summary),
            Detail::Failed(failed) => Some(&failed.resource),
            Detail::Closed => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DetailTab {
    Details,
    Events,
    Logs,
    Revisions,
    Argo,
    Actions,
    Yaml,
    Forward,
    Exec,
}

const TABS: &[(DetailTab, &str, &str)] = &[
    (DetailTab::Details, "show-details", "Details"),
    (DetailTab::Events, "show-events", "Events"),
    (DetailTab::Logs, "show-logs", "Logs"),
    (DetailTab::Revisions, "show-revisions", "Revisions"),
    (DetailTab::Argo, "show-argo", "Argo CD"),
    (DetailTab::Exec, "show-exec", "Exec"),
    (DetailTab::Forward, "show-forward", "Forward"),
    (DetailTab::Actions, "show-actions", "Actions"),
    (DetailTab::Yaml, "show-yaml", "YAML"),
];

/// `canShowLogs`, `canShowRevisions` and `isArgoApplication`; the other ported
/// tabs always show. Exec and Forward are not ported yet.
fn available(tab: DetailTab, resource: &ResourceSummary) -> bool {
    let namespaced = resource.namespace.is_some();
    match tab {
        DetailTab::Forward => namespaced && matches!(resource.kind.as_str(), "Pod" | "Service"),
        DetailTab::Exec => namespaced && resource.kind == "Pod",
        DetailTab::Logs => {
            namespaced && matches!(resource.kind.as_str(), "Pod" | "Deployment" | "Service")
        }
        DetailTab::Revisions => namespaced && resource.kind == "Deployment",
        DetailTab::Argo => {
            resource.kind == "Application"
                && resource
                    .api_version
                    .as_deref()
                    .is_some_and(|version| version.starts_with("argoproj.io/"))
        }
        _ => true,
    }
}

pub(super) struct ResourceDetail {
    pub data: ResourceDetailsFull,
    pub timeline_layout: u64,
    pub timeline_source: Arc<[u8]>,
    pub scroll: ScrollHandle,
    pub editor: Entity<EditorState>,
    pub marks: YamlMarks,
    pub lenses: Vec<super::yaml_lens::Lens>,
    pub lint: super::yaml_lint::Lint,
    pub tab: DetailTab,
    pub yaml: YamlApply,
    pub yaml_view: super::yaml_view::YamlView,
    pub secrets: super::secret_data::SecretData,
    pub events: EventsState,
    pub logs: Option<LogsState>,
    pub revisions: Option<RevisionsState>,
    /// The health assessment of the summary the panel opened for. The web's
    /// Argo CD tab reads `resource` (that summary), not the details read.
    pub opened_health: HealthAssessment,
    /// Whether the Details tab's health evidence disclosure is open.
    pub evidence_open: bool,
    pub argo: Option<ArgoTab>,
    pub actions: Option<ActionsState>,
    pub forward: super::forward_tab::ForwardForm,
    /// Expansion state of the Details metadata sections.
    pub metadata: MetadataDisclosure,
    _draft_changed: Subscription,
    _editor_changed: Subscription,
}

impl ResourceDetail {
    pub fn new(
        data: ResourceDetailsFull,
        window: &mut Window,
        cx: &mut Context<Inspector>,
    ) -> Self {
        let viewport = window.viewport_size();
        let timeline_source: Arc<[u8]> = serde_json::to_vec(&data)
            .expect("detail models serialize")
            .into();
        let timeline_layout = row_id((
            &timeline_source,
            f32::from(viewport.width).to_bits(),
            f32::from(viewport.height).to_bits(),
        ));
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("yaml")
                .line_number(true)
                // The web marks indentation with tints instead of guide lines.
                .indent_guides(false)
                .default_value(data.yaml.clone())
        });
        let marks = YamlMarks::new(&editor, cx);
        let editor_changed = cx.observe(&editor, |_, _, cx| cx.notify());
        let _draft_changed = cx.subscribe_in(&editor, window, |this, _, event, window, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            if let Detail::Ready(detail) = &mut this.detail
                && detail.yaml.draft_changed()
            {
                // A pending dry run reviewed the old text, so its result is dropped.
                this.diff_task.take();
            }
            this.schedule_yaml_lint(window, cx);
            cx.notify();
        });
        let forward = super::forward_tab::ForwardForm::new(
            &data.yaml,
            data.summary.kind == "Service",
            window,
            cx,
        );
        Self {
            metadata: Default::default(),
            lenses: Vec::new(),
            lint: Default::default(),
            secrets: Default::default(),
            forward,
            opened_health: data.summary.health_assessment.clone(),
            evidence_open: false,
            data,
            timeline_layout,
            timeline_source,
            scroll: ScrollHandle::default(),
            editor,
            marks,
            tab: DetailTab::Details,
            yaml: YamlApply::default(),
            yaml_view: super::yaml_view::YamlView::new(window, cx),
            events: EventsState::new(window, cx),
            logs: None,
            revisions: None,
            argo: None,
            actions: None,
            _draft_changed,
            _editor_changed: editor_changed,
        }
    }

    pub fn dirty(&self, cx: &App) -> bool {
        self.yaml.editing() && self.editor.read(cx).value().as_ref() != self.yaml.base
    }
}

impl Inspector {
    fn set_tab(&mut self, tab: DetailTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Detail::Ready(detail) = &mut self.detail {
            detail.secrets = Default::default();
            detail.tab = tab;
        }
        match tab {
            DetailTab::Logs => self.open_logs(window, cx),
            DetailTab::Revisions => self.open_revisions(window, cx),
            DetailTab::Argo => self.open_argo(window, cx),
            DetailTab::Actions => self.open_actions(window, cx),
            DetailTab::Yaml => self.reload_yaml_view(window, cx),
            _ => {}
        }
        cx.notify();
    }

    /// Opens a resource on a tab, as the GitOps page's Details button opens an
    /// Application on its Argo CD tab (`openGitOpsDetails` in `GitOpsSurface.svelte`).
    pub(super) fn open_resource_on(
        &mut self,
        resource: ResourceSummary,
        tab: Option<DetailTab>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_resource(resource, window, cx);
        if matches!(self.detail, Detail::Loading(_)) {
            self.detail_tab = tab;
        }
    }

    /// Replaces the loading panel with the read's result, then loads events and
    /// applies a tab `open_resource_on` requested, if the loaded resource shows it.
    pub(super) fn settle_detail(
        &mut self,
        result: Result<ResourceDetailsFull, AppError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let loading = match std::mem::replace(&mut self.detail, Detail::Closed) {
            Detail::Loading(resource) => Some(*resource),
            _ => None,
        };
        self.detail = match (result, loading) {
            (Ok(data), loading) => {
                let mut detail = ResourceDetail::new(data, window, cx);
                if let Some(resource) = loading {
                    detail.opened_health = resource.health_assessment;
                }
                Detail::Ready(Box::new(detail))
            }
            (Err(error), Some(resource)) => {
                Detail::Failed(Box::new(FailedDetail { resource, error }))
            }
            // Settling only follows a Loading state; without one there is no panel to show.
            (Err(_), None) => Detail::Closed,
        };
        if let Detail::Ready(detail) = &mut self.detail {
            detail.events.default_open = self.persisted.preferences.expand_timeline_by_default;
        }
        self.apply_yaml_defaults(cx, window);
        self.load_events(window, cx);
        let Some(tab) = self.detail_tab.take() else {
            return;
        };
        if let Detail::Ready(detail) = &self.detail
            && available(tab, &detail.data.summary)
        {
            self.set_tab(tab, window, cx);
        }
    }
}

impl Inspector {
    /// Moves the pane's edge with the pointer while the separator is held.
    pub(super) fn drag_detail_pane(
        &mut self,
        event: &MouseMoveEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_dragging {
            return;
        }
        if !event.dragging() {
            self.detail_dragging = false;
            return;
        }
        let percent = self.detail_percent_at(window, f32::from(event.position.x));
        self.set_detail_percent(percent);
        cx.notify();
    }

    /// `DetailPanelFrame.svelte`: the `w-2` resize separator (a centered
    /// `w-px bg-border/70` line under an `h-8 w-1` grip), then the `aside`.
    /// Pointer drag and the arrow, Home and End keys resize it.
    pub(super) fn detail_pane(&self, window: &Window, cx: &mut Context<Self>) -> [AnyElement; 2] {
        let handle = div()
            .id("detail-resize")
            .test_support()
            .track_focus(&self.detail_resize)
            .tab_stop(true)
            .role(gpui_kit::Role::Splitter)
            .cursor_col_resize()
            .aria_label("Resize details panel")
            .aria_numeric_value(f64::from(super::resources::detail_percent(self).round()))
            // Mirrors the numeric value as text, the only value the parity driver can read.
            .aria_value(format!(
                "{}",
                super::resources::detail_percent(self).round()
            ))
            .focus(|style| style.bg(Tokens::border().opacity(0.5)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.detail_dragging = true),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.resize_detail_key(event.keystroke.key.as_str()) {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(8.))
            .h_full()
            .child(div().w(px(1.)).h_full().bg(Tokens::border().opacity(0.7)))
            .child(
                div()
                    .absolute()
                    .w(px(4.))
                    .h(px(32.))
                    .rounded_full()
                    .bg(Tokens::border())
                    .opacity(0.7),
            );
        let aside = div()
            .flex_none()
            .w(px(super::resources::detail_width(self, window)))
            .h_full()
            .border_l_1()
            .border_color(Tokens::border())
            // `ResourceInspector.svelte`: `bg-surface-1` (= card).
            .bg(Tokens::card())
            .child(self.detail_view(cx));
        [handle.into_any_element(), aside.into_any_element()]
    }

    pub(super) fn detail_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let (name, subtitle) = match &self.detail {
            Detail::Closed => return div().into_any_element(),
            Detail::Loading(summary) => (summary.name.clone(), summary.kind.clone()),
            Detail::Ready(detail) => {
                let summary = &detail.data.summary;
                (
                    summary.name.clone(),
                    format!(
                        "{} / {}",
                        summary.kind,
                        summary.namespace.as_deref().unwrap_or("Cluster")
                    ),
                )
            }
            Detail::Failed(_) => ("Resource".into(), String::new()),
        };
        let active = match &self.detail {
            Detail::Ready(detail) => Some(detail.tab),
            _ => None,
        };
        let resource = self.detail.resource().cloned();
        let header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(8.))
            .px(px(16.))
            .py(px(12.))
            .border_b_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .min_w_0()
                    .child(ui::semibold(ui::sm(name)).truncate())
                    .child(ui::muted(ui::xs(subtitle)).truncate()),
            )
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(ui::ghost_icon("pin", 24., 14.))
                    .child(
                        ui::ghost_icon("x", 24., 14.)
                            .id("close-detail")
                            .test_support()
                            .aria_label("Close resource details")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.close_detail(cx))),
                    ),
            );
        // `TabsList variant="line" class="flex-wrap"`: an `h-8 p-[3px] gap-1` row of
        // 25px `px-1.5 text-xs font-medium` triggers; narrow panes wrap tabs.
        let tabs =
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .min_h(px(32.))
                .p(px(3.))
                .gap(px(4.))
                .children(
                    TABS.iter()
                        .filter(|(tab, ..)| {
                            resource
                                .as_ref()
                                .is_none_or(|resource| available(*tab, resource))
                        })
                        .map(|(tab, id, label)| {
                            let selected = active == Some(*tab);
                            let tab = *tab;
                            div()
                                .id(*id)
                                .test_support()
                                .role(gpui_kit::Role::Tab)
                                .aria_selected(selected)
                                .relative()
                                .flex()
                                .items_center()
                                .h(px(25.))
                                .px(px(6.))
                                .cursor_pointer()
                                .child(ui::medium(ui::xs(*label)).text_color(if selected {
                                    Tokens::foreground()
                                } else {
                                    Tokens::foreground().opacity(0.6)
                                }))
                                .when(selected, |trigger| {
                                    trigger.child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .right_0()
                                            .bottom(px(-4.))
                                            .h(px(2.))
                                            .bg(Tokens::foreground()),
                                    )
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.set_tab(tab, window, cx)
                                }))
                        }),
                );
        let body: AnyElement = match &self.detail {
            Detail::Closed => unreachable!("closed detail returned early"),
            Detail::Loading(_) => {
                ui::muted(ui::xs("Loading resource details...")).into_any_element()
            }
            Detail::Failed(failed) => {
                let permission =
                    friendly_error::resource_permission(&failed.resource, "get").map(|target| {
                        feedback::Permission {
                            cluster: failed.resource.cluster.clone(),
                            target,
                        }
                    });
                self.friendly_failure(
                    "detail-error",
                    Some(failed.error.kind),
                    &failed.error.message,
                    friendly_error::Context {
                        operation: Some(friendly_error::Operation::Details),
                        fallback_title: Some("Failed to load details"),
                        ..Default::default()
                    },
                    permission,
                    cx,
                )
            }
            Detail::Ready(detail) => {
                let summary = &detail.data.summary;
                let identity = format!(
                    "{} {} {}",
                    summary.kind,
                    summary.namespace.as_deref().unwrap_or("Cluster"),
                    summary.name
                );
                let content: AnyElement = match detail.tab {
                    DetailTab::Details => details_tab(
                        detail,
                        &self.timeline,
                        self.persisted.preferences.timestamp_timezone,
                        cx,
                    )
                    .into_any_element(),
                    DetailTab::Forward => self.forward_tab(cx).into_any_element(),
                    DetailTab::Exec => self.exec_view(cx),
                    DetailTab::Events => {
                        events_tab(detail, self.persisted.preferences.timestamp_timezone, cx)
                    }
                    DetailTab::Logs => logs_tab(detail, self.persisted.preferences.log_layout, cx),
                    DetailTab::Revisions => revisions_tab(detail, cx),
                    DetailTab::Argo => argo_tab(
                        detail,
                        self.persisted.preferences.timestamp_timezone,
                        self.argo_connection_panel(cx)
                            .children(self.argo_outcome_panel(detail, cx)),
                        detail
                            .argo
                            .as_ref()
                            .and_then(|s| s.inspector.as_ref())
                            .is_some_and(|s| self.argo_operation_busy(&s.application)),
                        super::argo_tab::ArgoConnectionState {
                            connected: self.argo_connection_id().is_some(),
                            unavailable: self.argo_connection_unavailable(),
                        },
                        &self.feedback.disclosure(),
                        cx,
                    ),
                    DetailTab::Actions => actions_tab(detail, cx),
                    DetailTab::Yaml => {
                        yaml_tab(self, detail, &self.persisted.preferences, cx).into_any_element()
                    }
                };
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .when(
                        matches!(
                            detail.tab,
                            DetailTab::Details | DetailTab::Events | DetailTab::Argo | DetailTab::Yaml
                        ),
                        |body| body.flex_none(),
                    )
                    .child(
                        div()
                            .id("detail-identity")
                            .test_support()
                            .aria_label(identity)
                            .absolute()
                            .size_0(),
                    )
                    .child(
                        div()
                            .id("detail-context")
                            .test_support()
                            .aria_label(summary.cluster.clone())
                            .absolute()
                            .size_0(),
                    )
                    .child(content)
                    .when_some(detail.events.watch_error.clone(), |body, error| body.child(
                        ui::xs(format!("Realtime watch failed: events: {error}. Showing the last loaded data."))
                            .text_color(Tokens::destructive())
                    ))
                    .into_any_element()
            }
        };
        let scroll = match &self.detail {
            Detail::Ready(detail) => Some(&detail.scroll),
            _ => None,
        };
        div()
            .id("detail")
            .flex()
            .flex_col()
            .size_full()
            // `ResourceInspector.svelte`: `section bg-surface-1`.
            .bg(Tokens::card())
            .child(header)
            // Tabs scroll with the body, as inside the web's `p-4` scroller (`Tabs gap-3`).
            .child(
                div()
                    .id("detail-body")
                    .test_support()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when_some(scroll, |body, scroll| body.track_scroll(scroll))
                    // The web capture shows content 8px from the panel's right
                    // edge despite `p-4`; the rest of the inset is the panel frame.
                    .pt(px(16.))
                    .pb(px(16.))
                    .pl(px(16.))
                    .pr(px(8.))
                    .gap(px(12.))
                    .child(tabs)
                    .child(body),
            )
            .into_any_element()
    }
}
