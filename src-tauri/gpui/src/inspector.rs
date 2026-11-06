mod action_drafts;
mod action_editor_view;
mod action_editors;
mod action_fields;
mod action_rollback_view;
mod actions_review;
mod actions_state;
mod actions_tab;
mod argo_connection;
mod argo_connection_view;
mod argo_highlight;
mod argo_operations;
mod argo_options;
mod argo_outcomes;
mod argo_panels;
mod argo_profiles;
mod argo_snapshot;
mod argo_tab;
mod argo_tunnel;
mod argo_workspace;
mod clipped;
mod cluster;
mod controls;
mod copyable;
mod custom_resources;
mod delivery_summary;
mod delivery_ui;
mod detail;
mod details_containers;
mod details_metadata;
mod details_status;
mod details_tab;
mod diagnostics;
mod events;
mod events_tab;
mod exec_buffer;
mod exec_custom;
mod exec_input;
mod exec_keys;
mod exec_leave;
mod exec_reflow;
mod exec_resize;
mod exec_selection;
mod exec_state;
mod exec_view;
mod feedback;
mod foreground_bar;
mod forward_tab;
mod friendly_error;
mod gitops_card;
mod gitops_nav;
mod gitops_page;
mod gitops_section;
mod gitops_tooltip;
mod header;
mod health_filter;
mod helm_card;
mod helm_details;
mod helm_page;
mod incident_guide;
mod incident_queue;
mod incidents;
mod kind_selection;
mod kubeconfig_reads;
mod launcher;
mod launcher_form;
mod launcher_transfer;
mod live_data;
pub use live_data::COALESCE as RESOURCE_WATCH_COALESCE;
mod live_events;
mod live_rows;
mod live_session_popover;
mod live_status;
mod log_row;
mod logs_state;
mod logs_tab;
mod map;
mod metrics;
mod namespace_list;
mod navigation_state;
mod overview;
mod overview_saved;
mod overview_shortcuts;
mod page_refresh;
mod palette;
mod port_forward_rows;
mod port_forwards;
mod preference_controls;
mod preferences;
mod preset_table;
mod rbac_inspector;
mod rbac_page;
mod rbac_queue;
mod rbac_review;
mod rbac_verify;
mod resource_filter_controls;
mod resource_filters;
mod resource_order;
mod resources;
mod restore_inventory;
mod restore_prompt;
mod revisions_tab;
mod row_index;
mod saved_forward_form;
mod scope;
mod secret_data;
mod session_actions;
mod session_cleanup;
mod settings_integrations;
mod settings_kubeconfig;
mod settings_mcp;
mod settings_page;
mod settings_sections;
mod sidebar;
mod sidebar_cache;
mod sidebar_discovery;
mod sidebar_reveal;
mod sidebar_tree;
mod surface;
mod table;
mod table_columns;
mod table_status;
#[cfg(test)]
#[path = "../tests/native/terminal_buffer.rs"]
mod terminal_buffer_tests;
mod timeline;
mod timestamp_text;
mod update_panel;
mod usage;
mod windowed;
mod workspace_editing;
mod workspace_entries;
mod workspace_reads;
mod yaml_apply;
mod yaml_lens;
mod yaml_lint;
mod yaml_marks;
mod yaml_tab;
mod yaml_view;

use crate::backend::{Backend, Scope};
use crate::theme::Tokens;
use crate::ui;
use detail::Detail;
use gpui_kit::TestSupportExt;
use gpui_kit::component::{
    IconName, IndexPath,
    input::{Input, InputEvent, InputState},
    select::{SearchableVec, SelectEvent, SelectState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, MouseButton, MouseMoveEvent, SharedString, Subscription,
    Task, Window, div, prelude::*, px,
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
    Namespaces,
}

/// The native workspace: sidebar navigation, header and the active page, laid
/// out like the Svelte `WorkspaceShell`.
pub struct Inspector {
    focus: gpui_kit::FocusHandle,
    backend: Arc<dyn Backend>,
    timings: crate::diagnostics::Timings,
    persisted: crate::preferences::Document,
    persistence_error: Option<String>,
    workspace_name: SharedString,
    workspace_read: Option<workspace_reads::OverviewRead>,
    saved_overview: overview_saved::SavedOverview,
    restore_inventory: Option<restore_inventory::RestoreInventory>,
    shortcut_owner: Option<(u64, String, String)>,
    scope: Scope,
    page: Page,
    context_picker: Entity<Choices>,
    search: Entity<InputState>,
    namespaces: Vec<NamespaceSummary>,
    custom_load: custom_resources::CustomLoad,
    table_sort: crate::table_sort::Sort,
    /// Group and kind-group keys the table has collapsed, as `collapsedGroups` on the web.
    collapsed_groups: std::collections::HashSet<String>,
    rows: Rc<crate::model::PreparedRows>,
    visible: crate::model::Visible,
    row_index: row_index::RowIndex,
    table_page: usize,
    table_rows: windowed::Windowed,
    timeline: timeline::State,
    load: Load,
    warnings: Vec<String>,
    detail: Detail,
    map: map::MapState,
    /// The map panel's collapse choice. `None` follows "Show ownership map by default".
    map_open: Option<bool>,
    table_open: bool,
    /// The detail pane's percent under the current size key (GitOps or not).
    detail_size: Option<f32>,
    detail_dragging: bool,
    detail_resize: gpui_kit::FocusHandle,
    metrics: metrics::MetricsState,
    usage: usage::UsageState,
    gitops: gitops_section::GitOpsState,
    gitops_page: gitops_page::GitOpsPage,
    argo_connections: argo_connection::Connections,
    argo_highlighter: Option<gpui_kit::component::highlighter::SyntaxHighlighter>,
    argo_outcomes: std::collections::HashMap<String, argo_outcomes::Outcome>,
    /// Submissions still in flight when their detail closed. Dropping them
    /// would cancel the submit and lose its outcome.
    argo_submissions: Vec<Task<()>>,
    helm_page: helm_page::HelmPage,
    /// A tab `open_resource_on` asked the loading detail panel to show.
    detail_tab: Option<detail::DetailTab>,
    cluster: cluster::ClusterState,
    incidents: incidents::IncidentsState,
    sessions: port_forwards::SessionsState,
    exec: exec_state::ExecState,
    rbac: rbac_page::RbacState,
    settings: settings_page::SettingsState,
    launcher: launcher::LauncherState,
    palette: palette::PaletteState,
    live_menu_open: bool,
    /// The compact-width navigation drawer; only shown below `sidebar::COMPACT_WIDTH`.
    navigation_open: bool,
    namespace_list: namespace_list::NamespaceListState,
    update_panel: update_panel::PanelState,
    live: live_data::LiveData,
    kind_selection: kind_selection::KindSelection,
    filters: resource_filters::ResourceFilters,
    sidebar_tree: sidebar_tree::TreeState,
    navigation: navigation_state::Memory,
    page_refresh: page_refresh::PageRefresh,
    notice: Option<String>,
    feedback: feedback::Feedback,
    restore_prompt: restore_prompt::RestorePrompt,
    session_cleanup: session_cleanup::SessionCleanup,
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
            cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() {
                    this.refresh_focused_page(cx);
                }
            }),
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
                    if this
                        .defer_exec_leave(exec_leave::LeaveIntent::Context(context.to_string()), cx)
                    {
                        return;
                    }
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
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut this = Self {
            focus,
            timings: Default::default(),
            backend,
            persisted: Default::default(),
            persistence_error: None,
            workspace_name: "KubeCove".into(),
            workspace_read: None,
            saved_overview: Default::default(),
            restore_inventory: None,
            shortcut_owner: None,
            scope: Scope::default(),
            page: Page::Overview,
            context_picker,
            search,
            namespaces: vec![],
            custom_load: Default::default(),
            table_sort: Default::default(),
            collapsed_groups: Default::default(),
            rows: Rc::default(),
            visible: Default::default(),
            row_index: Default::default(),
            table_page: 0,
            table_rows: windowed::Windowed::unwrapped(),
            timeline: Default::default(),
            load: Load::Loading,
            warnings: vec![],
            detail: Detail::Closed,
            map: map::MapState::new(),
            map_open: None,
            table_open: true,
            detail_size: None,
            detail_dragging: false,
            detail_resize: cx.focus_handle(),
            metrics: Default::default(),
            usage: Default::default(),
            gitops: Default::default(),
            gitops_page: gitops_page::GitOpsPage::new(),
            argo_connections: argo_connection::Connections::new(window, cx),
            argo_highlighter: None,
            argo_outcomes: Default::default(),
            argo_submissions: Vec::new(),
            helm_page: helm_page::HelmPage::new(window, cx),
            detail_tab: None,
            cluster: Default::default(),
            incidents: incidents::IncidentsState::new(),
            sessions: port_forwards::SessionsState::new(),
            exec: exec_state::ExecState::new(cx),
            rbac: rbac_page::RbacState::new(window, cx),
            settings: settings_page::SettingsState::new(window, cx),
            launcher: launcher::LauncherState::new(window, cx),
            palette: palette::PaletteState::new(window, cx),
            live_menu_open: false,
            navigation_open: false,
            namespace_list: Default::default(),
            update_panel: Default::default(),
            live: Default::default(),
            kind_selection: kind_selection::KindSelection::new(window, cx),
            filters: resource_filters::ResourceFilters::new(window, cx),
            sidebar_tree: Default::default(),
            navigation: Default::default(),
            page_refresh: Default::default(),
            notice: None,
            feedback: Default::default(),
            restore_prompt: Default::default(),
            session_cleanup: Default::default(),
            request: Rc::new(()),
            detail_request: Rc::new(()),
            context_task: None,
            scope_task: None,
            topology_task: None,
            detail_task: None,
            diff_task: None,
            _subscriptions: subscriptions,
        };
        this.load_preferences(cx);
        this.load_contexts(window, cx);
        // Without an open workspace the launcher is the startup view, so its own context and
        // namespace reads start here rather than on the first open.
        this.load_launcher_contexts(false, window, cx);
        // The web checks once at startup when updates are enabled (`scheduleAutomaticUpdateCheck`).
        this.check_settings_updates(window, cx);
        this
    }

    /// Opens a saved workspace named `name` on `context` at startup. The web driver creates its
    /// workspace through the launcher form, so this is the parity harness's setup seam.
    pub fn with_saved_workspace(
        mut self,
        name: impl Into<SharedString>,
        context: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        self.workspace_name = name.into();
        self.seed_saved_workspace(context.into(), cx);
        self
    }

    fn open_page(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page != page && self.defer_exec_leave(exec_leave::LeaveIntent::Page(page), cx) {
            return;
        }
        if self.page != page && self.can_leave(cx) {
            self.gitops_page.workspace = None;
            self.close_detail(cx);
            self.topology_task.take();
            self.set_page(page);
            if matches!(page, Page::Resources | Page::ClusterOverview) {
                self.load_topology(cx);
            }
            if page == Page::Overview {
                self.load_workspace_overview(false, None, cx);
            } else {
                self.workspace_read = None;
            }
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
        if self.defer_exec_leave(exec_leave::LeaveIntent::Namespace(namespace.clone()), cx) {
            return;
        }
        if !self.can_leave(cx) {
            return;
        }
        // Resource navigation starts unfiltered, as `openResources` does on the web.
        self.filters.health = resource_filters::HealthFilter::All;
        self.gitops_page.workspace = None;
        self.set_page(Page::Resources);
        self.sidebar_tree.selected = None;
        self.shortcut_owner = None;
        self.palette.kind = None;
        self.live.selected = None;
        self.live.kind_locked = false;
        self.kind_selection.chosen = None;
        // The sidebar scope replaces any toolbar namespace selection.
        self.filters.namespaces.clear();
        if let Some(namespace) = &namespace {
            self.visit_scope(
                crate::workspaces::entries::EntryKind::Namespace,
                self.scope.context.clone(),
                Some(namespace.clone()),
                namespace.clone(),
                cx,
            );
        }
        self.filter(cx);
        if self.scope.namespace != namespace {
            self.scope.namespace = namespace;
            self.load_scope(false, window, cx);
        } else {
            self.load_topology(cx);
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
        if self.defer_exec_leave(exec_leave::LeaveIntent::Close, cx) {
            return;
        }
        if !self.can_leave(cx) {
            return;
        }
        self.detail_task.take();
        self.diff_task.take();
        self.detail_request = Rc::new(());
        self.exec.clear();
        // Keep an in-flight submission alive so its outcome still reaches the observer.
        if let Detail::Ready(detail) = &mut self.detail
            && let Some(argo) = detail.argo.as_mut()
            && matches!(argo.operation.phase, argo_operations::Phase::Submitting)
            && let Some(task) = argo.operation.task.take()
        {
            self.argo_submissions.push(task);
        }
        self.detail = Detail::Closed;
        self.notice = None;
        cx.notify();
    }

    fn open_resource(
        &mut self,
        resource: ResourceSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.defer_exec_leave(
            exec_leave::LeaveIntent::Resource(Box::new(resource.clone())),
            cx,
        ) {
            return;
        }
        if !self.can_leave(cx) {
            return;
        }
        self.close_detail(cx);
        self.visit_resource(&resource, cx);
        self.detail = Detail::Loading(Box::new(resource.clone()));
        let request = self.detail_request.clone();
        let read = self.backend.details(resource);
        let read = self.timings.measure("get_resource_details", read);
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
            .bg(Tokens::input().opacity(0.3))
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
            Page::Namespaces => self.namespace_list_page(cx),
        }
    }
}

#[cfg(feature = "frame-profile")]
impl Inspector {
    pub fn frame_scroll_profile(&self) -> serde_json::Value {
        match &self.detail {
            Detail::Ready(detail) => serde_json::json!({
                "offset": f32::from(detail.scroll.offset().y),
                "maximum": f32::from(detail.scroll.max_offset().y),
                "bounds": format!("{:?}", detail.scroll.bounds()),
                "events": detail.events.events().len(),
            }),
            _ => serde_json::Value::Null,
        }
    }
}
impl Render for Inspector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _profile = crate::frame_profile::scope("inspector", "render");
        self.sync_argo_scope(window, cx);
        self.sync_sidebar_discovery(cx);
        self.sync_live(window, cx);
        self.flush_live_patches(cx);
        // `xl` in the web shell: the sidebar is in flow only at this width or wider.
        let compact = window.viewport_size().width < px(sidebar::COMPACT_WIDTH);
        if !compact {
            self.navigation_open = false;
        }
        self.restore_remembered_detail(window, cx);
        let detail_open = !matches!(self.detail, Detail::Closed);
        // Without an open workspace the web renders the launcher and settings only, so the shell's
        // sidebar, drawer and workspace pages stay out of the tree.
        let workspace = self.workspace_open();
        let page = if workspace || self.page == Page::Settings {
            self.page_view(window, cx)
        } else {
            div().into_any_element()
        };
        let overlays = self.overlays(window, cx);
        let sidebar = (workspace && !compact).then(|| {
            crate::frame_profile::element(
                "sidebar",
                self.sidebar(window, cx, sidebar::WIDE_WIDTH)
                    .into_any_element(),
            )
        });
        let drawer = (workspace && compact && self.navigation_open)
            .then(|| self.navigation_drawer(window, cx));
        div()
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .capture_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if event.keystroke.key != "escape" {
                        return;
                    }
                    if this.navigation_open {
                        this.navigation_open = false;
                        cx.stop_propagation();
                        cx.notify();
                    } else if this.close_argo_option_popovers(window, cx) {
                        cx.stop_propagation();
                        cx.notify();
                    } else if this.sessions.popover || this.update_panel.open {
                        this.sessions.popover = false;
                        this.close_update_panel();
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            )
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
            .child(self.foreground_bar())
            .children(sidebar)
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
                    .child(self.header(cx, compact))
                    .when_some(
                        self.persistence_error.clone().or(self.notice.clone()),
                        |main, notice| {
                            main.child(
                                div()
                                    .id("notice")
                                    .test_support()
                                    .aria_label(notice.clone())
                                    .role(gpui_kit::Role::Alert)
                                    .px(px(16.))
                                    .py(px(8.))
                                    .border_b_1()
                                    .border_color(Tokens::border())
                                    .child(ui::xs(notice)),
                            )
                        },
                    )
                    .children(self.restore_prompt(cx))
                    .children(self.session_cleanup_notice(cx))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .child(
                                div()
                                    .id("page")
                                    .test_support()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .overflow_y_scroll()
                                    .child(crate::frame_profile::element("page", page)),
                            )
                            .when(detail_open, |row| {
                                row.on_mouse_move(cx.listener(
                                    |this, event: &MouseMoveEvent, window, cx| {
                                        this.drag_detail_pane(event, window, cx);
                                    },
                                ))
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, _| this.detail_dragging = false),
                                )
                                .children(self.detail_pane(window, cx))
                            }),
                    )
                    .children(self.usage_footer(cx)),
            )
            .children(overlays)
            .children(drawer)
            .children(self.copy_toast(cx))
    }
}
