mod action_drafts;
mod action_editor_view;
mod action_editors;
mod action_fields;
mod action_rollback_view;
mod actions_review;
mod actions_state;
mod actions_tab;
mod argo_panels;
mod argo_snapshot;
mod argo_tab;
mod argo_workspace;
mod cluster;
mod controls;
mod custom_resources;
mod delivery_summary;
mod delivery_ui;
mod detail;
mod details_tab;
mod events;
mod events_tab;
mod gitops_card;
mod gitops_nav;
mod gitops_page;
mod gitops_section;
mod gitops_tooltip;
mod header;
mod helm_card;
mod helm_details;
mod helm_page;
mod incident_guide;
mod incident_queue;
mod incidents;
mod launcher;
mod launcher_form;
mod launcher_transfer;
mod live_data;
mod live_events;
mod live_rows;
mod live_status;
mod log_row;
mod logs_state;
mod logs_tab;
mod map;
mod metrics;
mod overview;
mod palette;
mod port_forwards;
mod rbac_inspector;
mod rbac_page;
mod rbac_queue;
mod rbac_review;
mod rbac_verify;
mod resources;
mod revisions_tab;
mod scope;
mod settings_integrations;
mod settings_kubeconfig;
mod settings_page;
mod settings_sections;
mod sidebar;
mod surface;
mod table;
mod timeline;
mod usage;
mod yaml_apply;
mod yaml_marks;
mod yaml_tab;
mod yaml_view;

use crate::backend::{Backend, Scope};
use crate::theme::Tokens;
use crate::ui;
use detail::Detail;
use gpui_kit::component::{
    IconName, IndexPath,
    input::{Input, InputEvent, InputState},
    select::{SearchableVec, SelectEvent, SelectState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, SharedString, Subscription, Task, Window, div, prelude::*, px,
};
use kubecove_lib::models::{NamespaceSummary, ResourceSummary};
use std::{rc::Rc, sync::Arc};

type Choices = SelectState<SearchableVec<SharedString>>;

enum Load {
    Loading,
    Ready,
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Overview,
    Resources,
    ClusterOverview,
    Incidents,
    PortForwards,
    Rbac,
    Settings,
    GitOps,
    Helm,
}

/// The native workspace: sidebar navigation, header and the active page, laid
/// out like the Svelte `WorkspaceShell`.
pub struct Inspector {
    backend: Arc<dyn Backend>,
    workspace_name: SharedString,
    scope: Scope,
    page: Page,
    context_picker: Entity<Choices>,
    search: Entity<InputState>,
    namespaces: Vec<NamespaceSummary>,
    rows: Vec<ResourceSummary>,
    visible: Vec<ResourceSummary>,
    table_page: usize,
    load: Load,
    warnings: Vec<String>,
    detail: Detail,
    map: map::MapState,
    metrics: metrics::MetricsState,
    usage: usage::UsageState,
    gitops: gitops_section::GitOpsState,
    gitops_page: gitops_page::GitOpsPage,
    helm_page: helm_page::HelmPage,
    /// A tab `open_resource_on` asked the loading detail panel to show.
    detail_tab: Option<detail::DetailTab>,
    cluster: cluster::ClusterState,
    incidents: incidents::IncidentsState,
    sessions: port_forwards::SessionsState,
    rbac: rbac_page::RbacState,
    settings: settings_page::SettingsState,
    launcher: launcher::LauncherState,
    palette: palette::PaletteState,
    live_menu_open: bool,
    live: live_data::LiveData,
    notice: Option<String>,
    request: Rc<()>,
    detail_request: Rc<()>,
    context_task: Option<Task<()>>,
    scope_task: Option<Task<()>>,
    topology_task: Option<Task<()>>,
    detail_task: Option<Task<()>>,
    diff_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Inspector {
    pub fn new(backend: Arc<dyn Backend>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let context_picker = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(Vec::<SharedString>::new()),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
            .searchable(true)
        });
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(
                "Search by name, namespace, kind, owner, GitOps owner, Helm release...",
            )
        });
        let owner = cx.entity().downgrade();
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.table_page = 0;
                    this.filter(cx);
                }
            }),
            cx.subscribe_in(&context_picker, window, |this, _, event, window, cx| {
                let SelectEvent::Confirm(Some(context)) = event else {
                    return;
                };
                if this.scope.context != context.as_ref() {
                    this.scope = Scope {
                        context: context.to_string(),
                        namespace: None,
                    };
                    this.load_scope(false, window, cx);
                }
            }),
        ];
        window.on_window_should_close(cx, move |_, cx| {
            owner
                .update(cx, |this, cx| this.can_leave(cx))
                .unwrap_or(true)
        });
        let mut this = Self {
            backend,
            workspace_name: "KubeCove".into(),
            scope: Scope::default(),
            page: Page::Overview,
            context_picker,
            search,
            namespaces: vec![],
            rows: vec![],
            visible: vec![],
            table_page: 0,
            load: Load::Loading,
            warnings: vec![],
            detail: Detail::Closed,
            map: map::MapState::new(),
            metrics: Default::default(),
            usage: Default::default(),
            gitops: Default::default(),
            gitops_page: gitops_page::GitOpsPage::new(),
            helm_page: helm_page::HelmPage::new(window, cx),
            detail_tab: None,
            cluster: Default::default(),
            incidents: incidents::IncidentsState::new(),
            sessions: port_forwards::SessionsState::new(),
            rbac: rbac_page::RbacState::new(window, cx),
            settings: settings_page::SettingsState::new(window, cx),
            launcher: launcher::LauncherState::new(window, cx),
            palette: palette::PaletteState::new(window, cx),
            live_menu_open: false,
            live: Default::default(),
            notice: None,
            request: Rc::new(()),
            detail_request: Rc::new(()),
            context_task: None,
            scope_task: None,
            topology_task: None,
            detail_task: None,
            diff_task: None,
            _subscriptions: subscriptions,
        };
        this.load_contexts(window, cx);
        this
    }

    /// Names the workspace shown in the sidebar header and overview.
    pub fn with_workspace_name(mut self, name: impl Into<SharedString>) -> Self {
        self.workspace_name = name.into();
        self
    }

    fn open_page(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page != page && self.can_leave(cx) {
            self.gitops_page.workspace = None;
            self.close_detail(cx);
            self.page = page;
            self.load_page(cx);
            cx.notify();
        }
    }

    fn select_namespace(
        &mut self,
        namespace: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_leave(cx) {
            return;
        }
        self.gitops_page.workspace = None;
        self.page = Page::Resources;
        self.palette.kind = None;
        self.live.selected = None;
        self.live.kind_locked = false;
        self.filter(cx);
        if self.scope.namespace != namespace {
            self.scope.namespace = namespace;
            self.load_scope(false, window, cx);
        }
        cx.notify();
    }

    fn dirty(&self, cx: &App) -> bool {
        matches!(&self.detail, Detail::Ready(detail) if detail.dirty(cx))
    }

    fn can_leave(&mut self, cx: &mut Context<Self>) -> bool {
        if self.dirty(cx) {
            self.notice = Some(
                "This local draft has unsaved changes. Discard the local draft before changing target or closing.".into(),
            );
            cx.notify();
            false
        } else {
            true
        }
    }

    fn close_detail(&mut self, cx: &mut Context<Self>) {
        if !self.can_leave(cx) {
            return;
        }
        self.detail_task.take();
        self.diff_task.take();
        self.detail_request = Rc::new(());
        self.detail = Detail::Closed;
        self.rebuild_map();
        self.notice = None;
        cx.notify();
    }

    fn open_resource(
        &mut self,
        resource: ResourceSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_leave(cx) {
            return;
        }
        self.close_detail(cx);
        self.detail = Detail::Loading(Box::new(resource.clone()));
        self.rebuild_map();
        let request = self.detail_request.clone();
        let read = self.backend.details(resource);
        self.detail_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !Rc::ptr_eq(&this.detail_request, &request) {
                    return;
                }
                this.settle_detail(result, window, cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn search_input(&self) -> impl IntoElement {
        Input::new(&self.search)
            .id("search")
            .aria_label("Search resources")
            .prefix(
                gpui_kit::component::Icon::new(IconName::Search)
                    .size_4()
                    .text_color(Tokens::muted_foreground()),
            )
            .bg(Tokens::background().opacity(0.7))
            .text_xs()
            .w_full()
    }

    fn page_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        match self.page {
            Page::Overview => self.overview(cx).into_any_element(),
            Page::Resources | Page::ClusterOverview => {
                self.resources(window, cx).into_any_element()
            }
            Page::Incidents => self.incidents_page(window, cx).into_any_element(),
            Page::PortForwards => self.port_forwards_page(cx).into_any_element(),
            Page::Rbac => self.rbac_page(window, cx).into_any_element(),
            Page::Settings => self.settings_page(cx).into_any_element(),
            Page::GitOps => self.gitops_page_view(window, cx),
            Page::Helm => self.helm_page_view(window, cx),
        }
    }
}

impl Render for Inspector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_live(window, cx);
        self.flush_live_patches(cx);
        let detail_open = !matches!(self.detail, Detail::Closed);
        let page = self.page_view(window, cx);
        let overlays = self.overlays(window, cx);
        div()
            .relative()
            .size_full()
            .flex()
            .gap(px(8.))
            .px(px(8.))
            // The web's 2px in-flow `ForegroundLoadingBar` sits above its `h-screen p-2`
            // shell, shifting it down 2px and clipping 2px of bottom padding.
            .pt(px(10.))
            .pb(px(6.))
            .bg(Tokens::background())
            .text_color(Tokens::foreground())
            .font_family(crate::theme::SANS)
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::background())
                    .child(self.header(cx))
                    .when_some(self.notice.clone(), |main, notice| {
                        main.child(
                            div()
                                .id("notice")
                                .role(gpui_kit::Role::Alert)
                                .px(px(16.))
                                .py(px(8.))
                                .border_b_1()
                                .border_color(Tokens::border())
                                .child(ui::xs(notice)),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .child(
                                div()
                                    .id("page")
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .overflow_y_scroll()
                                    .child(page),
                            )
                            .when(detail_open, |row| {
                                row.children(self.detail_pane(window, cx))
                            }),
                    )
                    .children(self.usage_footer(cx)),
            )
            .children(overlays)
    }
}
