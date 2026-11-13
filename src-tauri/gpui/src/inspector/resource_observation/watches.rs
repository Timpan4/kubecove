//! Watch starts and held changes belong to the observation they describe.
use super::ResourceObservation;
use crate::backend::{CLUSTER_KINDS, Scope};
use crate::inspector::{Inspector, Page};
use crate::model::ObservedNames;
use futures_util::future::{BoxFuture, FutureExt, join_all};
use gpui_kit::Context;
use kubecove_lib::models::{ResourceSummary, WatchResourceKey};
use std::collections::HashSet;

pub(in crate::inspector) struct Patch {
    pub scope: Scope,
    pub key: WatchResourceKey,
    pub rows: Vec<ResourceSummary>,
    pub resource_version: Option<String>,
    pub names: Option<ObservedNames>,
    pub watch: Option<String>,
}

impl ResourceObservation {
    pub(in crate::inspector) fn accepts(&self, scope: &Scope) -> bool {
        scope.namespace == self.namespace && self.has_context(&scope.context)
    }

    pub(in crate::inspector) fn queue(&mut self, patch: Patch) {
        self.pending.push(patch);
    }

    pub(in crate::inspector) fn discard_patches(&mut self) {
        self.pending.clear();
    }

    pub(in crate::inspector) fn retain_watches(&mut self, wanted: &HashSet<&str>) {
        self.pending
            .retain(|patch| patch.watch.as_deref().is_none_or(|id| wanted.contains(id)));
        self.starts.retain(|id, _| wanted.contains(id.as_str()));
    }

    /// Each patch waits for its own lists and for shared row preparation.
    pub(in crate::inspector) fn ready_patches(&mut self) -> Vec<Patch> {
        if self.holds_patches() {
            return Vec::new();
        }
        let mut pending = std::mem::take(&mut self.pending);
        pending.retain(|patch| self.accepts(&patch.scope));
        let (held, ready) = pending.into_iter().partition(|patch: &Patch| {
            self.context_loading(&patch.scope.context)
                || (patch.key.resource_kind.api_version.is_some()
                    && self.custom_loading(&patch.scope.context))
        });
        self.pending = held;
        ready
    }
}

impl Inspector {
    pub(in crate::inspector) fn watch_ready(&self, context: &str, key: &WatchResourceKey) -> bool {
        self.rows_settled(context, key)
            || matches!(
                self.observation
                    .starts
                    .get(&crate::backend::watch_id(context, key)),
                Some(Some(_))
            )
    }

    pub(in crate::inspector) fn watch_resume_version(
        &self,
        context: &str,
        key: &WatchResourceKey,
    ) -> Option<String> {
        match self
            .observation
            .starts
            .get(&crate::backend::watch_id(context, key))
        {
            Some(Some(start)) if !self.rows_settled(context, key) => Some(start.clone()),
            _ => self.shown_version(context, key),
        }
    }

    /// Reads the start version of each built-in kind the page watches in each
    /// of `contexts`, in their order. Each entry settles once that context's
    /// versions are read, so the caller lists its rows after them; `None`
    /// when the context reads none. Custom kinds and cluster kinds come from
    /// later reads, so their watches still wait for those rows.
    pub(in crate::inspector) fn read_watch_starts(
        &mut self,
        contexts: &[String],
        cx: &mut Context<Self>,
    ) -> Vec<Option<BoxFuture<'static, ()>>> {
        let keys: Vec<_> = self
            .live_keys()
            .filter(|_| self.focused_application().is_none())
            .unwrap_or_default()
            .into_iter()
            .filter(|key| {
                key.resource_kind.plural.is_none()
                    && !CLUSTER_KINDS.contains(&key.resource_kind.kind.as_str())
            })
            .collect();
        contexts
            .iter()
            .map(|context| {
                // The Cluster Overview watches the selected context alone.
                if self.page == Page::ClusterOverview && *context != self.scope.context {
                    return None;
                }
                let scope = Scope {
                    context: context.clone(),
                    namespace: self.scope.namespace.clone(),
                };
                let reads: Vec<_> = keys
                    .iter()
                    .filter_map(|key| {
                        let read = self
                            .backend
                            .watch_start_version(scope.clone(), key.clone())?
                            .shared();
                        let id = crate::backend::watch_id(context, key);
                        self.observation.starts.insert(id.clone(), None);
                        let version = read.clone();
                        // A failed read leaves the watch to wait for its rows.
                        self.observation
                            .start_tasks
                            .push(cx.spawn(async move |this, cx| {
                                let version = version.await;
                                let _ = this.update(cx, |this, cx| {
                                    match version {
                                        Ok(version) => {
                                            if let Some(slot) = this.observation.starts.get_mut(&id)
                                            {
                                                *slot = Some(version);
                                            }
                                        }
                                        Err(_) => {
                                            this.observation.starts.remove(&id);
                                        }
                                    }
                                    cx.notify();
                                });
                            }));
                        Some(read)
                    })
                    .collect();
                (!reads.is_empty()).then(|| join_all(reads).map(|_| ()).boxed())
            })
            .collect()
    }
}
