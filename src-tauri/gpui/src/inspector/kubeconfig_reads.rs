//! Kubeconfig source and context reads for the workspace launcher and the cluster picker.
//! A successful read is reused while fresh, as the web's query cache reuses it. Contexts are
//! keyed by the source the web passes to `list_kube_contexts`, and Retry always refetches.

use super::Inspector;
use crate::backend::KubeconfigSourcesSummary;
use crate::read_retry;
use gpui_kit::{Context, Task, Window};
use kubecove_lib::models::{AppError, ClusterContext};
use std::time::{Duration, Instant};

/// `kubeconfigSourcesQueryOptions` `staleTime` in `src/lib/kubeconfig-sources-query.ts`.
const SOURCE_STALE_TIME: Duration = Duration::from_millis(60_000);
/// The launcher's contexts query `staleTime` in `WorkspaceLauncher.svelte`.
const CONTEXTS_STALE_TIME: Duration = Duration::from_millis(30_000);

#[derive(Default)]
pub(super) struct KubeconfigReads {
    source: Option<(Instant, KubeconfigSourcesSummary)>,
    context_list: Option<CachedContexts>,
}

struct CachedContexts {
    read_at: Instant,
    source_key: Option<String>,
    rows: Vec<ClusterContext>,
}

impl KubeconfigReads {
    fn fresh_source(&self, now: Instant) -> Option<KubeconfigSourcesSummary> {
        self.source
            .as_ref()
            .filter(|(read_at, _)| now.duration_since(*read_at) < SOURCE_STALE_TIME)
            .map(|(_, source)| source.clone())
    }

    fn fresh_contexts(&self, now: Instant) -> Option<(Option<String>, Vec<ClusterContext>)> {
        self.context_list
            .as_ref()
            .filter(|cached| now.duration_since(cached.read_at) < CONTEXTS_STALE_TIME)
            .map(|cached| (cached.source_key.clone(), cached.rows.clone()))
    }
}

impl Inspector {
    /// Reads the kubeconfig source, then the contexts for that source, and passes the contexts
    /// (or their error) to `done` with any source error. A fresh cached value is reused unless
    /// `force` is set. Failed reads are not cached, so the next open refetches them.
    pub(super) fn read_kubeconfig_contexts(
        &mut self,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
        done: impl FnOnce(
            &mut Self,
            Result<Vec<ClusterContext>, AppError>,
            Option<String>,
            &mut Window,
            &mut Context<Self>,
        ) + 'static,
    ) -> Task<()> {
        let now = Instant::now();
        let reads = &self.launcher.kubeconfig;
        let cached_source = (!force).then(|| reads.fresh_source(now)).flatten();
        let cached_contexts = (!force).then(|| reads.fresh_contexts(now)).flatten();
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let mut fetched_source = None;
            let (source_key, source_error) = match cached_source {
                Some(source) => (Some(source.source_key), None),
                None => {
                    let backend = backend.clone();
                    match read_retry::read(executor.clone(), move || backend.kubeconfig_sources())
                        .await
                    {
                        Ok(source) => {
                            let key = source.source_key.clone();
                            fetched_source = Some(source);
                            (Some(key), None)
                        }
                        Err(error) => (None, Some(error.message)),
                    }
                }
            };
            // A source error still reads the default contexts, as the web's query does.
            let (result, fetched_contexts) = match cached_contexts {
                Some((key, rows)) if key == source_key => (Ok(rows), false),
                _ => {
                    let key = source_key.clone();
                    let read = read_retry::read(executor, move || backend.contexts(key.clone()));
                    let read = timings.measure("list_kube_contexts", read);
                    (read.await, true)
                }
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let reads = &mut this.launcher.kubeconfig;
                let read_at = Instant::now();
                // A failed fetch evicts the older success, so the next open retries
                // instead of showing that success as this read's result.
                if let Some(source) = fetched_source {
                    reads.source = Some((read_at, source));
                } else if source_error.is_some() {
                    reads.source = None;
                }
                if fetched_contexts {
                    reads.context_list = result.as_ref().ok().map(|rows| CachedContexts {
                        read_at,
                        source_key: source_key.clone(),
                        rows: rows.clone(),
                    });
                }
                done(this, result, source_error, window, cx);
            });
        })
    }
}
