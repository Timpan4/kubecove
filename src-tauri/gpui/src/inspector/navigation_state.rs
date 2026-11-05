//! Session-only navigation memory for open workspaces. Leaving a workspace keeps
//! its page, search, kind and namespace filters and the open resource detail
//! with its tab, and returning restores them. Nothing here reaches preferences
//! or disk, so an app restart starts from the default view.

use super::detail::{Detail, DetailTab};
use super::{Inspector, Load, Page};
use crate::backend::Scope;
use gpui_kit::{Context, Window};
use kubecove_lib::models::ResourceSummary;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct Snapshot {
    context: String,
    page: Page,
    search: String,
    namespace: Option<String>,
    kinds: Option<HashSet<String>>,
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
    /// Records the active workspace before another workspace replaces it.
    pub(super) fn remember_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.launcher.active else {
            return;
        };
        let detail = match &self.detail {
            Detail::Ready(detail) => Some((detail.data.summary.clone(), detail.tab)),
            Detail::Loading(resource) => Some((
                (**resource).clone(),
                self.detail_tab.unwrap_or(DetailTab::Details),
            )),
            Detail::Closed | Detail::Failed(_) => None,
        };
        let snapshot = Snapshot {
            context: self.scope.context.clone(),
            page: self.page,
            search: self.search.read(cx).value().to_string(),
            namespace: self.scope.namespace.clone(),
            kinds: self.kind_selection.chosen.clone(),
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
        let search = saved
            .as_ref()
            .map(|saved| saved.search.clone())
            .unwrap_or_default();
        self.search
            .update(cx, |input, cx| input.set_value(search, window, cx));
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
            || self.live.scope_loading
            || matches!(self.load, Load::Loading)
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
