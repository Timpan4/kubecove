//! The resource kinds watched by the current page.
use super::{Inspector, Page};
use kubecove_lib::models::WatchResourceKey;

impl Inspector {
    /// The keys the page watches in each of its contexts, or `None` when it
    /// watches nothing.
    pub(super) fn live_keys(&self) -> Option<Vec<WatchResourceKey>> {
        if !self.workspace_open()
            || self.scope.context.is_empty()
            || !matches!(
                self.page,
                Page::Resources | Page::ClusterOverview | Page::Overview
            )
        {
            return None;
        }
        if let Some(kind) = &self.live.selected {
            return Some(vec![crate::backend::discovered_key(
                kind,
                self.scope.namespace.clone(),
            )]);
        }
        Some(
            self.available_kind_keys()
                .into_iter()
                .filter(|(id, _, _)| self.kind_selected(id))
                .map(|(_, _, key)| key)
                .collect(),
        )
    }
}
