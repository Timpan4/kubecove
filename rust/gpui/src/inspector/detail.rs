//! Resource detail panel, mirroring `ResourceDetailPanel.svelte`. Each tab
//! lives in its own module; tabs load their data the first time they open.

use super::Inspector;
use super::actions::{ActionsState, actions_tab};
use super::argo::ArgoTab;
use super::details_tab::{MetadataDisclosure, details_tab};
use super::events::EventsState;
use super::events_tab::events_tab;
use super::feedback;
use super::friendly_error;
use super::logs_state::LogsState;
use super::logs_tab::logs_tab;
use super::revisions_tab::{RevisionsState, revisions_tab};
use super::windowed::row_id;
use super::yaml::{self, YamlApply, YamlMarks, yaml_tab};
use crate::control_motion;
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::component::input::{EditorState, InputEvent};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, KeyDownEvent, MouseButton, MouseMoveEvent,
    ScrollHandle, SharedString, Subscription, Task, Window, div, prelude::*, px,
};
use kubecove_lib::models::{AppError, HealthAssessment, ResourceDetailsFull, ResourceSummary};
use std::sync::Arc;

mod pane;

pub(super) enum Detail {
    Closed,
    Loading(Box<LoadingDetail>),
    Failed(Box<FailedDetail>),
    Ready(Box<ResourceDetail>),
}

/// A details read in flight. Leaving this state drops the read, which cancels it,
/// and drops `tab`, so a closed panel's tab never reaches the next resource.
pub(super) struct LoadingDetail {
    pub resource: ResourceSummary,
    /// The tab `open_resource_on` asked for or the user picked while the read
    /// runs, shown once the read succeeds.
    pub tab: Option<DetailTab>,
    _read: Task<()>,
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
            Detail::Loading(loading) => Some(&loading.resource),
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
    pub lenses: Vec<yaml::Lens>,
    pub lint: yaml::Lint,
    pub tab: DetailTab,
    pub yaml: YamlApply,
    pub yaml_view: yaml::YamlView,
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
        // The timeline projects only the summary and the status conditions, so
        // they alone key it, not the whole document with its YAML.
        let timeline_source: Arc<[u8]> = serde_json::to_vec(&(&data.summary, &data.status))
            .expect("detail models serialize")
            .into();
        let timeline_layout = row_id((
            &timeline_source,
            f32::from(viewport.width).to_bits(),
            f32::from(viewport.height).to_bits(),
        ));
        let (factory, syntax) = yaml::fold_factory();
        let editor = cx.new(|cx| {
            let mut editor = yaml::viewer(data.yaml.clone(), window, cx)
                // The web marks indentation with tints instead of guide lines.
                .indent_guides(false);
            editor.set_highlighter_factory(factory, cx);
            editor
        });
        let marks = YamlMarks::new(&editor, cx);
        let editor_changed = cx.observe(&editor, |_, _, cx| cx.notify());
        let _draft_changed = cx.subscribe_in(&editor, window, |this, _, event, window, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            if let Detail::Ready(detail) = &mut this.detail {
                detail.yaml.draft_changed();
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
            lint: yaml::Lint::new(syntax),
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
            yaml_view: yaml::YamlView::new(window, cx),
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
        self.yaml.dirty(self.editor.read(cx).value().as_ref())
    }
}

impl Inspector {
    fn set_tab(&mut self, tab: DetailTab, window: &mut Window, cx: &mut Context<Self>) {
        self.request_view();
        match &mut self.detail {
            Detail::Ready(detail) => {
                detail.secrets = Default::default();
                detail.tab = tab;
            }
            // `settle_detail` opens the tab picked while the read runs.
            Detail::Loading(loading) => {
                loading.tab = Some(tab);
                cx.notify();
                return;
            }
            Detail::Closed | Detail::Failed(_) => {}
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
        if self.defer_exec_leave(
            super::exec::LeaveIntent::Resource(Box::new(resource.clone()), tab),
            cx,
        ) {
            return;
        }
        if !self.can_leave(cx) {
            return;
        }
        self.close_detail(cx);
        self.visit_resource(&resource, cx);
        let read = self.backend.details(resource.clone());
        let read = self.timings.measure("get_resource_details", read);
        let read = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settle_detail(result, window, cx);
                cx.notify();
            });
        });
        self.detail = Detail::Loading(Box::new(LoadingDetail {
            resource,
            tab,
            _read: read,
        }));
        cx.notify();
    }

    /// Replaces the loading panel with the read's result, then loads events and
    /// applies the tab `open_resource_on` requested, if the loaded resource shows it.
    fn settle_detail(
        &mut self,
        result: Result<ResourceDetailsFull, AppError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let loading = match std::mem::replace(&mut self.detail, Detail::Closed) {
            Detail::Loading(loading) => *loading,
            other => {
                self.detail = other;
                return;
            }
        };
        let LoadingDetail { resource, tab, .. } = loading;
        self.detail = match result {
            Ok(data) => {
                let mut detail = ResourceDetail::new(data, window, cx);
                detail.opened_health = resource.health_assessment;
                detail.events.default_open = self.persisted.preferences.expand_timeline_by_default;
                Detail::Ready(Box::new(detail))
            }
            Err(error) => Detail::Failed(Box::new(FailedDetail { resource, error })),
        };
        self.apply_yaml_defaults(cx, window);
        self.load_events(window, cx);
        if let Some(tab) = tab
            && let Detail::Ready(detail) = &self.detail
            && available(tab, &detail.data.summary)
        {
            self.set_tab(tab, window, cx);
        }
    }
}
