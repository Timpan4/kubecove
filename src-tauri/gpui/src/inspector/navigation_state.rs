//! Session-only navigation memory for open workspaces. Leaving a workspace keeps
//! its page, sidebar node, kind, namespace, expanded sidebar branches and the
//! open resource detail with its tab, and returning restores them. The table,
//! toolbar and panel state in `Browser` comes back only when the sidebar selects
//! no single kind, and the health and owner filters follow the same rule, as the
//! web does (`fixedResourceKindScope`). Nothing here reaches preferences or disk,
//! so an app restart starts from the default view.

use super::detail::{Detail, DetailTab};
use super::resource_filters::HealthFilter;
use super::{Inspector, Load, Page};
use crate::backend::Scope;
use crate::table_sort::Sort;
use gpui_kit::{Context, Window};
use kubecove_lib::models::{DiscoveredResourceKind, ResourceSummary};
use std::collections::{HashMap, HashSet};

/// The resource browser state the web keeps as `PathStateResourceBrowserState`.
#[derive(Clone)]
struct Browser {
    search: String,
    namespaces: Vec<String>,
    sort: Sort,
    page: usize,
    collapsed: HashSet<String>,
    /// The effective map and table panel states, so an unset map preference
    /// restores as the value the user saw.
    map_open: bool,
    table_open: bool,
    network: bool,
}

#[derive(Clone)]
struct Snapshot {
    context: String,
    page: Page,
    browser: Browser,
    /// The expanded sidebar branches, as `expandedSections` on the web.
    sidebar_open: HashSet<String>,
    namespace: Option<String>,
    kinds: Option<HashSet<String>>,
    /// Whether the sidebar selected one kind. The web keeps the browser state,
    /// health and GitOps filters out of such a restore (`fixedResourceKindScope`).
    kind_node: bool,
    health: HealthFilter,
    gitops_owner: Option<String>,
    /// The sidebar row path, as `WorkspaceNavigationState.selectedNode` on the web.
    sidebar_node: Option<String>,
    /// The custom kind a namespace's Custom Resources row selected, and whether
    /// the kind is locked to that row.
    custom_kind: Option<DiscoveredResourceKind>,
    custom_kind_locked: bool,
    detail: Option<(ResourceSummary, DetailTab)>,
}

#[derive(Default)]
pub(super) struct Memory {
    workspaces: HashMap<u64, Snapshot>,
    /// The detail to reopen once the restored scope read has settled.
    pending_detail: Option<(ResourceSummary, DetailTab)>,
}

impl Memory {
    pub(super) fn forget(&mut self, id: u64) {
        self.workspaces.remove(&id);
    }
}

fn same_resource(row: &ResourceSummary, target: &ResourceSummary) -> bool {
    row.cluster == target.cluster
        && row.kind == target.kind
        && row.namespace == target.namespace
        && row.name == target.name
}

impl Inspector {
    /// Whether the sidebar row selects one kind, as the web's
    /// `selectedNode.type === "kind"`: a built-in kind row, whose path ends in
    /// its kind, or a custom kind row, which locks `live`. Category rows such as
    /// Workloads also set `kind_selection.chosen` but keep the browser state.
    fn sidebar_kind_node(&self) -> bool {
        let kind_row = self
            .sidebar_tree
            .selected
            .as_deref()
            .and_then(|path| path.rsplit('/').next())
            .is_some_and(|leaf| {
                super::sidebar_tree::GROUPS
                    .iter()
                    .any(|(_, kinds)| kinds.contains(&leaf))
            });
        matches!(self.page, Page::Resources | Page::ClusterOverview)
            && self.sidebar_tree.selected.is_some()
            && (self.live.kind_locked || (kind_row && self.kind_selection.chosen.is_some()))
    }

    /// Records the active workspace before another workspace replaces it.
    pub(super) fn remember_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.launcher.active else {
            return;
        };
        let detail = match &self.detail {
            Detail::Ready(detail) => Some((detail.data.summary.clone(), detail.tab)),
            Detail::Loading(loading) => Some((
                loading.resource.clone(),
                loading.tab.unwrap_or(DetailTab::Details),
            )),
            Detail::Closed | Detail::Failed(_) => None,
        };
        let browser = Browser {
            search: self.search.read(cx).value().to_string(),
            namespaces: self.filters.namespaces.clone(),
            sort: self.table_sort,
            page: self.table_page,
            collapsed: self.collapsed_groups.clone(),
            map_open: self.map_is_open(),
            table_open: self.table_open,
            network: self.map.network,
        };
        let snapshot = Snapshot {
            context: self.scope.context.clone(),
            page: self.page,
            browser,
            sidebar_open: self.sidebar_tree.open.clone(),
            namespace: self.scope.namespace.clone(),
            kinds: self.kind_selection.chosen.clone(),
            kind_node: self.sidebar_kind_node(),
            health: self.filters.health,
            gitops_owner: self.filters.gitops_owner.clone(),
            sidebar_node: self.sidebar_tree.selected.clone(),
            custom_kind: self.live.selected.clone(),
            custom_kind_locked: self.live.kind_locked,
            detail,
        };
        self.navigation.workspaces.insert(id, snapshot);
    }

    /// Applies the workspace's remembered page and filters before its scope loads.
    /// A workspace opened for the first time starts on Overview with no filters.
    pub(super) fn restore_workspace_memory(
        &mut self,
        id: u64,
        context: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let saved = self
            .navigation
            .workspaces
            .get(&id)
            .filter(|saved| saved.context == context)
            .cloned();
        self.page = saved.as_ref().map_or(Page::Overview, |saved| saved.page);
        self.scope = Scope {
            context: context.to_owned(),
            namespace: saved.as_ref().and_then(|saved| saved.namespace.clone()),
        };
        self.kind_selection.chosen = saved.as_ref().and_then(|saved| saved.kinds.clone());
        // A single-kind sidebar node restores none of the browser state, as the
        // web's `kindScopeLocked` drops its path state.
        let restored = saved.as_ref().filter(|saved| !saved.kind_node);
        self.filters.health = restored.map_or(HealthFilter::All, |saved| saved.health);
        self.filters.gitops_owner = restored.and_then(|saved| saved.gitops_owner.clone());
        let browser = restored.map(|saved| saved.browser.clone());
        self.sidebar_tree.selected = saved.as_ref().and_then(|saved| saved.sidebar_node.clone());
        self.live.selected = saved.as_ref().and_then(|saved| saved.custom_kind.clone());
        self.live.kind_locked = saved.as_ref().is_some_and(|saved| saved.custom_kind_locked);
        self.sidebar_tree.open = saved
            .as_ref()
            .map(|saved| saved.sidebar_open.clone())
            .unwrap_or_default();
        let search = browser
            .as_ref()
            .map_or_else(String::new, |b| b.search.clone());
        self.search
            .update(cx, |input, cx| input.set_value(search, window, cx));
        self.filters.namespaces = browser
            .as_ref()
            .map_or_else(Vec::new, |b| b.namespaces.clone());
        self.table_sort = browser.as_ref().map_or_else(Sort::default, |b| b.sort);
        self.table_page = browser.as_ref().map_or(0, |b| b.page);
        self.collapsed_groups = browser
            .as_ref()
            .map_or_else(HashSet::new, |b| b.collapsed.clone());
        self.map_open = browser.as_ref().map(|b| b.map_open);
        self.table_open = browser.as_ref().is_none_or(|b| b.table_open);
        self.map.network = browser.as_ref().is_some_and(|b| b.network);
        self.navigation.pending_detail = saved.and_then(|saved| saved.detail);
    }

    /// Reopens the remembered detail after its restored scope read settles.
    /// The remembered detail is dropped when the resource is no longer listed,
    /// when the read failed, or when the restored page does not list resources.
    pub(super) fn restore_remembered_detail(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.navigation.pending_detail.is_none()
            || self.scope_load.loading()
            || matches!(self.scope_load.rows, Load::Loading)
        {
            return;
        }
        let Some((target, tab)) = self.navigation.pending_detail.take() else {
            return;
        };
        if !matches!(self.page, Page::Resources | Page::ClusterOverview)
            || !matches!(self.detail, Detail::Closed)
        {
            return;
        }
        let listed = self
            .visible
            .iter()
            .find(|row| same_resource(row, &target))
            .cloned();
        if let Some(row) = listed {
            self.open_resource_on(row, Some(tab), window, cx);
        }
    }
}
