//! Kubeconfig source and context reads for the workspace launcher and the cluster picker.
//! A successful read is reused while fresh, as the web's query cache reuses it. Contexts are
//! keyed by the source the web passes to `list_kube_contexts`, and Retry always refetches.

use super::request::Fetch;
use super::{Inspector, Page};
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
    /// Raised by every saved source change, so a read that started before it cannot store
    /// what it read for the previous source.
    generation: u64,
}

struct CachedContexts {
    read_at: Instant,
    source_key: Option<String>,
    rows: Vec<ClusterContext>,
}

impl KubeconfigReads {
    /// Stores the source a saved change returned, as `KubeconfigSettings.svelte` writes it to
    /// the query cache with `setQueryData`. Returns whether its source key differs from the
    /// cached one: contexts are keyed by source, so only then do they need a new read.
    pub(super) fn replace_source(&mut self, source: KubeconfigSourcesSummary) -> bool {
        let changed = self
            .source
            .as_ref()
            .is_none_or(|(_, cached)| cached.source_key != source.source_key);
        self.generation += 1;
        self.source = Some((Instant::now(), source));
        changed
    }

    /// Changes with every saved source, so a read can tell that a newer source replaced the
    /// one it read.
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

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
    /// Stores the source a saved change returned. When its source key changed, the reads keyed
    /// by the source run again, as the web's queries keyed by `kubeconfigSourceKey` do: the
    /// contexts of the launcher and the context picker, the page reads, and the open
    /// workspace's rows when a page showing them opens. The open workspace keeps its saved
    /// context, so a context the new source lacks shows as unavailable instead of being
    /// replaced. The next live-session read stops sessions outside the new source unless they
    /// are kept, as the web's cleanup effect does. A label-only change keeps the key, so it
    /// refreshes nothing.
    pub(super) fn apply_source_change(
        &mut self,
        source: KubeconfigSourcesSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.launcher.kubeconfig.replace_source(source) {
            return;
        }
        for page in [Page::GitOps, Page::Helm, Page::Rbac, Page::Incidents] {
            self.invalidate_page_read(page);
        }
        self.settings.integrations.scope_reread = true;
        self.load_sessions(cx);
        // The web's header hides its picker while the new source's contexts load and shows
        // their error if the read fails, so no context of the previous source stays selectable.
        let context = self.scope.context.clone();
        self.set_context_picker(Vec::new(), &context, window, cx);
        // Replacing the launcher's read drops one still running for the previous source.
        let read = self.read_kubeconfig_contexts(
            false,
            window,
            cx,
            |this, result, source_warning, window, cx| {
                if let Ok(contexts) = &result {
                    let selected = if this.workspace_open() {
                        this.scope.context.clone()
                    } else {
                        contexts
                            .iter()
                            .find(|context| context.is_current)
                            .or(contexts.first())
                            .map(|context| context.name.clone())
                            .unwrap_or_default()
                    };
                    this.set_context_picker(contexts.clone(), &selected, window, cx);
                }
                this.apply_launcher_contexts(result, source_warning, window, cx);
            },
        );
        self.launcher.contexts = Fetch::start(read);
        cx.notify();
    }

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
        let generation = reads.generation;
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
                // A source saved since this read started is newer than anything it read.
                if reads.generation == generation {
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
                }
                done(this, result, source_error, window, cx);
            });
        })
    }
}
