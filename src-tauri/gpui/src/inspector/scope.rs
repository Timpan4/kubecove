//! Context and resource-scope reads, refresh, and visible resource filtering.

use super::load_trace::{LoadKind, LoadTrace};
use super::scope_load::{all_failed, union_namespaces};
use super::{Inspector, Load, map};
use crate::backend::{Read, Scope, ScopeResult};
use crate::model::PreparedRows;
use crate::workspaces::reads::{ContextResults, Settled, settle};
use futures_util::{FutureExt, future::Shared};
use gpui_kit::component::select::SearchableVec;
use gpui_kit::{BackgroundExecutor, Context, SharedString, Window};
use kubecove_lib::models::{AppError, ResourceSummary};
use std::{rc::Rc, time::Instant};

/// Every context's read with the rows taken out, and those rows prepared.
struct ScopeRows {
    results: ContextResults<ScopeResult>,
    rows: PreparedRows,
}

/// Prepares the rows of every context that answered on the background
/// executor, in context rank order, so applying the read on the UI thread
/// only swaps them in. Fails only when every context failed.
async fn prepare_off_thread(
    executor: &BackgroundExecutor,
    contexts: Vec<String>,
    mut results: ContextResults<ScopeResult>,
) -> Result<ScopeRows, AppError> {
    if !results.is_empty() && results.iter().all(|(_, result)| result.is_err()) {
        return Err(all_failed(results));
    }
    // Results come in plan order, so the rows come in context rank order.
    // Joining them copies every row, so it runs with the preparation.
    let (results, rows) = executor
        .spawn(async move {
            let mut resources = Vec::new();
            for (_, result) in &mut results {
                if let Ok(result) = result {
                    resources.append(&mut result.resources);
                }
            }
            let rows = PreparedRows::with_inherited_owners(resources, &contexts);
            (results, rows)
        })
        .await;
    Ok(ScopeRows { results, rows })
}

/// The builtin and custom reads of every workspace context, shareable while in
/// flight. Dropping every clone cancels the reads, as dropping one owned read did.
#[derive(Clone)]
pub(super) struct SharedScopeReads {
    /// The contexts read, in rank order.
    pub(super) contexts: Vec<String>,
    builtin: Shared<Settled<ScopeResult>>,
    custom: Option<Shared<Settled<ScopeResult>>>,
}

impl SharedScopeReads {
    /// The Overview's read of these contexts: each context's built-ins, then
    /// its custom kinds, merged as the backend's combined read merges them,
    /// then the contexts merged as the Overview's own read merges them.
    pub(super) fn overview_read(&self) -> Read<ScopeResult> {
        let builtin = self.builtin.clone();
        let custom = self.custom.clone();
        Box::pin(async move {
            let mut results = builtin.await;
            if let Some(custom) = custom {
                for (context, custom) in custom.await {
                    let Some((_, slot)) = results.iter_mut().find(|(other, _)| *other == context)
                    else {
                        continue;
                    };
                    let Ok(result) = slot else {
                        continue;
                    };
                    match custom {
                        Ok(custom) => {
                            result.resources.extend(custom.resources);
                            result.warnings.extend(custom.warnings);
                            result.listed.extend(custom.listed);
                        }
                        // The backend's combined read fails with its custom kinds.
                        Err(error) => *slot = Err(error),
                    }
                }
            }
            crate::workspaces::reads::merge(results)
        })
    }
}

impl Inspector {
    pub(super) fn load_contexts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.context_task.take();
        self.scope_load.rows = Load::Loading;
        let task = self.read_kubeconfig_contexts(
            false,
            window,
            cx,
            |this, result, _source_error, window, cx| {
                match result {
                    Ok(contexts) if contexts.is_empty() => {
                        this.scope_load.rows = Load::Failed(
                            "No Kubernetes contexts found. Configure KUBECONFIG and retry.".into(),
                        )
                    }
                    Ok(contexts) => {
                        this.launcher.context_choices = contexts
                            .iter()
                            .map(|context| (context.name.clone(), context.is_current))
                            .collect();
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
                        if this.workspace_open() {
                            this.load_scope(false, window, cx);
                        }
                    }
                    Err(error) => this.scope_load.rows = Load::Failed(error.message),
                }
                cx.notify();
            },
        );
        self.context_task = Some(task);
    }

    pub(super) fn load_scope(
        &mut self,
        refresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.scope_trace = Some(LoadTrace::start(
            LoadKind::Scope,
            &self.scope.context,
            self.scope.namespace.as_deref(),
        ));
        self.scope_load.cancel();
        self.sync_page_reads(cx);
        self.kind_selection.reads.clear();
        self.live.reset_scope();
        let contexts = self.resource_contexts();
        self.discover_kinds(contexts.clone(), cx);
        self.close_detail(cx);
        self.request = Rc::new(());
        let request = self.request.clone();
        // Shared, so the Overview's read of the same contexts joins this request instead of issuing another.
        let reads = self.scope_reads(&contexts, refresh, cx);
        self.load_workspace_overview(refresh, Some(reads.clone()), cx);
        // As the web's keepPreviousData does, rows of the same contexts stay visible until the new read settles.
        if !(self.scope_load.same_contexts(&contexts) && !self.rows.is_empty()) {
            self.scope_load.rows = Load::Loading;
            self.rows = Rc::default();
            self.warnings.clear();
        }
        self.prepare_rows();
        self.filter(cx);
        let SharedScopeReads {
            builtin, custom, ..
        } = reads;
        let awaits_custom = custom.is_some();
        let plan = contexts.clone();
        // One task, so dropping it (a newer scope or refresh) cancels the custom read too.
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = prepare_off_thread(cx.background_executor(), plan, builtin.await).await;
            let applied = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return false;
                }
                let apply_started = Instant::now();
                let awaits_custom = custom.is_some();
                let ready = match result {
                    Ok(ScopeRows { results, rows }) => {
                        let applied = this.scope_load.apply_parts(results, false);
                        this.namespaces = applied.namespaces;
                        this.rows = Rc::new(rows);
                        this.warnings = applied.warnings;
                        this.scope_load.builtin_settled(Ok(()));
                        true
                    }
                    Err(error) => {
                        this.live.refresh_error = Some(error.message.clone());
                        this.scope_load.builtin_settled(Err(error.message));
                        this.rows = Rc::default();
                        this.warnings.clear();
                        false
                    }
                };
                this.prepare_rows();
                this.filter(cx);
                cx.notify();
                // A pending custom read still has to apply before the load is final.
                let complete = !(ready && awaits_custom);
                this.note_apply(LoadKind::Scope, apply_started.elapsed(), complete);
                ready
            });
            let Some(custom) = custom else {
                return;
            };
            if !matches!(applied, Ok(true)) {
                return;
            }
            let result = custom.await;
            let _ = this.update(cx, |this, cx| {
                if Rc::ptr_eq(&this.request, &request) {
                    let apply_started = Instant::now();
                    this.apply_custom_resources(result, cx);
                    this.note_apply(LoadKind::Scope, apply_started.elapsed(), true);
                }
            });
        });
        self.scope_load.start(contexts, awaits_custom, task);
        // The map loads on its own, like the web's topology query, so a
        // topology failure leaves the table usable.
        self.load_metrics(window, cx);
        self.load_topology(cx);
        self.load_inventory(refresh, cx);
        self.load_page(cx);
        cx.notify();
    }

    pub(super) fn set_map_mode(
        &mut self,
        network: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.map.network == network {
            return;
        }
        self.map.network = network;
        self.load_topology(cx);
    }

    pub(super) fn load_topology(&mut self, cx: &mut Context<Self>) {
        self.topology_task.take();
        if !matches!(
            self.page,
            super::Page::Resources | super::Page::ClusterOverview
        ) {
            return;
        }
        self.map.cache.take();
        // The map shows one workspace context, chosen in the "Map context" select.
        let scope = Scope {
            context: self.map_context(),
            namespace: self.scope.namespace.clone(),
        };
        self.map_trace = Some(LoadTrace::start(
            LoadKind::Map,
            &scope.context,
            scope.namespace.as_deref(),
        ));
        self.map.replace_topology(map::TopologyLoad::Loading);
        let request = self.request.clone();
        let network = self.map.network;
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        self.topology_task = Some(cx.spawn(async move |this, cx| {
            let result = crate::read_retry::cancelled_read(
                || {
                    let read = if network {
                        backend.network_topology(scope.clone())
                    } else {
                        backend.topology(scope.clone())
                    };
                    timings.measure("list_resource_topology", read)
                },
                |delay| cx.background_executor().timer(delay),
            )
            .await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) || this.map.network != network {
                    return;
                }
                let apply_started = Instant::now();
                this.map.replace_topology(match result {
                    Ok(topology) => map::TopologyLoad::Ready(topology),
                    Err(error) => map::TopologyLoad::Failed(error),
                });
                // The resource render rebuilds the map before drawing it, once
                // per frame, so a scope read applied in the same frame costs no
                // second build.
                cx.notify();
                this.note_apply(LoadKind::Map, apply_started.elapsed(), true);
            });
        }));
        cx.notify();
    }

    pub(super) fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _span = crate::frame_profile::scope("scope", "refresh_start");
        self.live_menu_open = false;
        if self.scope.context.is_empty() {
            self.load_contexts(window, cx)
        } else {
            self.live.refresh_error = None;
            self.scope_trace = Some(LoadTrace::start(
                LoadKind::Scope,
                &self.scope.context,
                self.scope.namespace.as_deref(),
            ));
            let contexts = self.resource_contexts();
            let read = self.refresh_reads(&contexts, cx);
            let request = self.request.clone();
            let task = cx.spawn_in(window, async move |this, cx| {
                let result =
                    prepare_off_thread(cx.background_executor(), contexts, read.await).await;
                let _ = this.update(cx, |this, cx| {
                    if !Rc::ptr_eq(&this.request, &request) {
                        return;
                    }
                    let apply_started = Instant::now();
                    match result {
                        Ok(ScopeRows { results, rows }) => {
                            let span = crate::frame_profile::scope("scope", "apply_parts");
                            let applied = this.scope_load.apply_parts(results, true);
                            drop(span);
                            let span = crate::frame_profile::scope("scope", "swap_rows");
                            this.rows = Rc::new(this.keep_failed_contexts(rows, &applied.kept));
                            drop(span);
                            this.namespaces = if applied.kept.is_empty() {
                                applied.namespaces
                            } else {
                                union_namespaces(applied.namespaces, &this.namespaces)
                            };
                            this.warnings = applied.warnings;
                            this.scope_load.refresh_settled(Ok(()));
                            let span = crate::frame_profile::scope("scope", "prepare_rows");
                            this.prepare_rows();
                            drop(span);
                            this.filter(cx);
                        }
                        Err(error) => {
                            this.live.refresh_error = Some(error.message.clone());
                            this.scope_load.refresh_settled(Err(error.message));
                        }
                    }
                    cx.notify();
                    this.note_apply(LoadKind::Scope, apply_started.elapsed(), true);
                });
            });
            self.scope_load.start_refresh(task);
            self.load_topology(cx);
            self.load_inventory(true, cx);
            self.load_page(cx);
        }
        cx.notify();
    }

    pub(super) fn filter(&mut self, cx: &mut Context<Self>) {
        let _span = crate::frame_profile::scope("scope", "filter");
        self.update_saved_overview();
        self.filter_rows(cx);
        // The page render rebuilds the map once per frame, so several filters
        // before a frame, or one before a new topology replaces the old, cost one.
        self.map.links_stale = true;
        cx.notify();
    }

    /// Resolves cluster-row inheritance before the filter source joins it with scope rows.
    pub(super) fn prepare_rows(&mut self) {
        let mut prepared = self.cluster.rows.clone();
        crate::model::inherit_gitops_owners(&mut prepared);
        self.cluster.prepared = prepared;
    }

    /// The resource scope read of every context in two stages: built-ins
    /// first, then custom kinds. Every context starts at once: each one is its
    /// own API server, and the backend already bounds the lists within one.
    /// A focused GitOps application keeps its single application read.
    fn scope_reads(
        &self,
        contexts: &[String],
        refresh: bool,
        cx: &Context<Self>,
    ) -> SharedScopeReads {
        if self.focused_application().is_some() {
            let read = self.application_resources_read(refresh, cx);
            let read = self.timings.measure("list_resource_scope", read);
            return SharedScopeReads {
                contexts: contexts.to_vec(),
                builtin: settle(vec![(self.scope.context.clone(), read)]).shared(),
                custom: None,
            };
        }
        let mut builtin = Vec::with_capacity(contexts.len());
        let mut custom = Vec::new();
        for context in contexts {
            let scope = Scope {
                context: context.clone(),
                namespace: self.scope.namespace.clone(),
            };
            if let Some(first) = self.backend.scope_custom_resources(scope.clone()) {
                let backend = self.backend.clone();
                let scope = scope.clone();
                let mut first = Some(first);
                let read = crate::read_retry::read(cx.background_executor().clone(), move || {
                    first.take().unwrap_or_else(|| {
                        backend
                            .scope_custom_resources(scope.clone())
                            .expect("custom read remains available")
                    })
                });
                custom.push((context.clone(), read));
            }
            let backend = self.backend.clone();
            let mut refresh = refresh;
            let read = crate::read_retry::read(cx.background_executor().clone(), move || {
                backend.scope_builtin_resources(scope.clone(), std::mem::take(&mut refresh))
            });
            let read = self.timings.measure("list_resource_scope", read);
            builtin.push((context.clone(), read));
        }
        SharedScopeReads {
            contexts: contexts.to_vec(),
            builtin: settle(builtin).shared(),
            custom: (!custom.is_empty()).then(|| settle(custom).shared()),
        }
    }

    /// The live menu's refresh of every context: built-ins and custom kinds in
    /// one read per context. A focused GitOps application keeps its single read.
    fn refresh_reads(&self, contexts: &[String], cx: &Context<Self>) -> Settled<ScopeResult> {
        if self.focused_application().is_some() {
            let read = self.application_resources_read(true, cx);
            let read = self.timings.measure("list_resource_scope", read);
            return settle(vec![(self.scope.context.clone(), read)]);
        }
        let reads = contexts
            .iter()
            .map(|context| {
                let backend = self.backend.clone();
                let scope = Scope {
                    context: context.clone(),
                    namespace: self.scope.namespace.clone(),
                };
                let mut refresh = true;
                let read = crate::read_retry::read(cx.background_executor().clone(), move || {
                    backend.resources(scope.clone(), std::mem::take(&mut refresh))
                });
                (
                    context.clone(),
                    self.timings.measure("list_resource_scope", read),
                )
            })
            .collect();
        settle(reads)
    }

    /// Rows in the saved workspace kinds, which the overview summarizes.
    pub(super) fn workspace_rows(&self) -> impl Iterator<Item = &ResourceSummary> {
        self.workspace_read
            .as_ref()
            .map_or(&self.rows[..], |state| state.rows.as_slice())
            .iter()
            .filter(|row| self.launcher.overview_kind(&row.kind) && self.launcher.in_scope(row))
    }

    /// Kinds in the resource scope: the page's kinds plus custom kinds present.
    pub(super) fn scope_kinds(&self) -> Vec<String> {
        let mut kinds: Vec<String> = self
            .base_kinds()
            .iter()
            .map(|kind| (*kind).to_owned())
            .collect();
        if let Some(chosen) = &self.kind_selection.chosen {
            for (key, label, _) in self.available_kind_keys() {
                if chosen.contains(&key) && !kinds.contains(&label) {
                    kinds.push(label);
                }
            }
        }
        if !self.persisted.preferences.show_custom_resources {
            return kinds;
        }
        for row in self.rows.iter() {
            if self.persisted.preferences.show_custom_resources
                && row.dynamic == Some(true)
                && !kinds.contains(&row.kind)
            {
                kinds.push(row.kind.clone());
            }
        }
        for kind in &self.live.kinds {
            if !kinds.contains(&kind.kind) {
                kinds.push(kind.kind.clone());
            }
        }
        kinds
    }
}
