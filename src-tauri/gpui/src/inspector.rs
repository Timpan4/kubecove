mod actions_review;
mod actions_state;
mod actions_tab;
mod cluster;
mod controls;
mod detail;
mod details_tab;
mod events;
mod events_tab;
mod gitops_section;
mod header;
mod incident_guide;
mod incident_queue;
mod incidents;
mod log_row;
mod logs_state;
mod logs_tab;
mod map;
mod overview;
mod port_forwards;
mod resources;
mod revisions_tab;
mod sidebar;
mod surface;
mod table;
mod timeline;
mod yaml_apply;
mod yaml_marks;
mod yaml_tab;

use crate::backend::{Backend, Scope};
use crate::model::{WORKSPACE_KINDS, ownership_group};
use crate::theme::Tokens;
use crate::ui;
use detail::{Detail, ResourceDetail};
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
    gitops: gitops_section::GitOpsState,
    cluster: cluster::ClusterState,
    incidents: incidents::IncidentsState,
    sessions: port_forwards::SessionsState,
    live_menu_open: bool,
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
            gitops: Default::default(),
            cluster: Default::default(),
            incidents: incidents::IncidentsState::new(),
            sessions: port_forwards::SessionsState::new(),
            live_menu_open: false,
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

    fn load_contexts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.context_task.take();
        self.load = Load::Loading;
        let read = self.backend.contexts();
        self.context_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(contexts) if contexts.is_empty() => {
                        this.load = Load::Failed(
                            "No Kubernetes contexts found. Configure KUBECONFIG and retry.".into(),
                        )
                    }
                    Ok(contexts) => {
                        let selected = contexts
                            .iter()
                            .find(|context| context.is_current)
                            .unwrap_or(&contexts[0])
                            .name
                            .clone();
                        this.context_picker.update(cx, |state, cx| {
                            state.set_items(
                                SearchableVec::new(
                                    contexts
                                        .into_iter()
                                        .map(|context| SharedString::from(context.name))
                                        .collect::<Vec<_>>(),
                                ),
                                window,
                                cx,
                            );
                            state.set_selected_value(
                                &SharedString::from(selected.clone()),
                                window,
                                cx,
                            );
                        });
                        this.scope.context = selected;
                        this.load_scope(false, window, cx);
                    }
                    Err(error) => this.load = Load::Failed(error.message),
                }
                cx.notify();
            });
        }));
    }

    fn load_scope(&mut self, refresh: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.scope_task.take();
        self.close_detail(cx);
        self.request = Rc::new(());
        let request = self.request.clone();
        self.load = Load::Loading;
        self.rows.clear();
        self.warnings.clear();
        self.filter(cx);
        let read = self.backend.resources(self.scope.clone(), refresh);
        self.scope_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return;
                }
                match result {
                    Ok(result) => {
                        this.namespaces = result.namespaces;
                        this.rows = result.resources;
                        this.warnings = result.warnings;
                        this.load = Load::Ready;
                    }
                    Err(error) => this.load = Load::Failed(error.message),
                }
                this.filter(cx);
                cx.notify();
            });
        }));
        // The map loads on its own, like the web's topology query, so a
        // topology failure leaves the table usable.
        self.map.topology = map::TopologyLoad::Loading;
        let request = self.request.clone();
        let read = self.backend.topology(self.scope.clone());
        self.topology_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return;
                }
                this.map.topology = match result {
                    Ok(topology) => map::TopologyLoad::Ready(topology),
                    Err(error) => map::TopologyLoad::Failed(error.message),
                };
                cx.notify();
            });
        }));
        self.load_inventory(refresh, cx);
        self.load_page(cx);
        cx.notify();
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.live_menu_open = false;
        if self.scope.context.is_empty() {
            self.load_contexts(window, cx)
        } else {
            self.load_scope(true, window, cx)
        }
    }

    fn filter(&mut self, cx: &mut Context<Self>) {
        let search = self.search.read(cx).value().trim().to_lowercase();
        let kinds = self.scope_kinds();
        self.visible = self
            .rows
            .iter()
            .chain(self.cluster_rows())
            .filter(|row| kinds.contains(&row.kind))
            .filter(|row| {
                search.is_empty()
                    || row.name.to_lowercase().contains(&search)
                    || row.kind.to_lowercase().contains(&search)
                    || row
                        .namespace
                        .as_deref()
                        .is_some_and(|ns| ns.to_lowercase().contains(&search))
                    || ownership_group(row).0.to_lowercase().contains(&search)
                    || row
                        .helm_release
                        .as_deref()
                        .is_some_and(|release| release.to_lowercase().contains(&search))
            })
            .cloned()
            .collect();
        cx.notify();
    }

    /// Rows in the saved workspace kinds, which the overview summarizes.
    fn workspace_rows(&self) -> impl Iterator<Item = &ResourceSummary> {
        self.rows
            .iter()
            .filter(|row| WORKSPACE_KINDS.contains(&row.kind.as_str()))
    }

    /// Kinds in the resource scope: the page's kinds plus custom kinds present.
    fn scope_kinds(&self) -> Vec<String> {
        let mut kinds: Vec<String> = self
            .base_kinds()
            .iter()
            .map(|kind| (*kind).to_owned())
            .collect();
        for row in &self.rows {
            if row.dynamic == Some(true) && !kinds.contains(&row.kind) {
                kinds.push(row.kind.clone());
            }
        }
        kinds
    }

    fn open_page(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page != page && self.can_leave(cx) {
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
        self.page = Page::Resources;
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
        let request = self.detail_request.clone();
        let read = self.backend.details(resource);
        self.detail_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !Rc::ptr_eq(&this.detail_request, &request) {
                    return;
                }
                this.detail = match result {
                    Ok(data) => Detail::Ready(Box::new(ResourceDetail::new(data, window, cx))),
                    Err(error) => Detail::Failed(error.message),
                };
                this.load_events(window, cx);
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
        }
    }
}

impl Render for Inspector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let detail_open = !matches!(self.detail, Detail::Closed);
        let page = self.page_view(window, cx);
        div()
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
                                row.child(
                                    div()
                                        .flex_none()
                                        .w(gpui_kit::relative(0.395))
                                        .h_full()
                                        .border_l_1()
                                        .border_color(Tokens::border())
                                        // `ResourceInspector.svelte`: `bg-surface-1` (= card).
                                        .bg(Tokens::card())
                                        .child(self.detail_view(cx)),
                                )
                            }),
                    ),
            )
    }
}
