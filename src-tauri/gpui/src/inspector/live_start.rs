//! Watches that open while their rows load. Before the view lists a
//! context's built-in kinds, it reads the current list version of each kind it
//! watches there. Each watch opens at its version as soon as it is read, and
//! the list starts only after every version is read, so the rows hold each
//! change before the watch's first event.
use super::{Inspector, Page};
use crate::backend::{CLUSTER_KINDS, Scope};
use futures_util::future::{BoxFuture, FutureExt, join_all};
use gpui_kit::Context;
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

    /// Reads the start version of each built-in kind the page watches in each
    /// of `contexts`, in their order. Each entry settles once that context's
    /// versions are read, so the caller lists its rows after them; `None`
    /// when the context reads none. Custom kinds and cluster kinds come from
    /// later reads, so their watches still wait for those rows.
    pub(super) fn read_watch_starts(
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
                        self.live.starts.insert(id.clone(), None);
                        let version = read.clone();
                        // A failed read leaves the watch to wait for its rows.
                        self.live.start_tasks.push(cx.spawn(async move |this, cx| {
                            let version = version.await;
                            let _ = this.update(cx, |this, cx| {
                                match version {
                                    Ok(version) => {
                                        if let Some(slot) = this.live.starts.get_mut(&id) {
                                            *slot = Some(version);
                                        }
                                    }
                                    Err(_) => {
                                        this.live.starts.remove(&id);
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
