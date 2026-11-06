//! Context and resource-scope reads, refresh, and visible resource filtering.

use super::custom_resources::CustomLoad;
use super::{Inspector, Load, map};
use crate::backend::Read;
use crate::backend::ScopeResult;
use futures_util::{FutureExt, future::Shared};
use gpui_kit::component::select::SearchableVec;
use gpui_kit::{Context, SharedString, Window};
use kubecove_lib::models::ResourceSummary;
use std::rc::Rc;

/// The builtin and custom reads of one resource scope, shareable while in flight.
/// Dropping every clone cancels the reads, as dropping one owned read did.
#[derive(Clone)]
pub(super) struct SharedScopeReads {
    builtin: Shared<Read<ScopeResult>>,
    custom: Option<Shared<Read<ScopeResult>>>,
}

impl SharedScopeReads {
    /// The Overview's read of this scope: built-ins, then custom kinds, merged as
    /// the backend's combined read merges them.
    pub(super) fn overview_read(&self) -> Read<ScopeResult> {
        let builtin = self.builtin.clone();
        let custom = self.custom.clone();
        Box::pin(async move {
            let mut result = builtin.await?;
            if let Some(custom) = custom {
                let custom = custom.await?;
                result.resources.extend(custom.resources);
                result.warnings.extend(custom.warnings);
            }
            Ok(result)
        })
    }
}

impl Inspector {
    pub(super) fn load_contexts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.context_task.take();
        self.load = Load::Loading;
        let task = self.read_kubeconfig_contexts(
            false,
            window,
            cx,
            |this, result, _source_error, window, cx| {
                match result {
                    Ok(contexts) if contexts.is_empty() => {
                        this.load = Load::Failed(
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
                    Err(error) => this.load = Load::Failed(error.message),
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
        self.scope_task.take();
        self.sync_page_reads(cx);
        self.kind_selection.reads.clear();
        self.live.reset_scope();
        self.live.scope_loading = true;
        self.discover_kinds(cx);
        self.close_detail(cx);
        self.request = Rc::new(());
        let request = self.request.clone();
        let (builtin, custom) = self.scope_reads(refresh, cx);
        // Shared, so the Overview's read of the same scope joins this request instead of issuing another.
        let builtin = self
            .timings
            .measure("list_resource_scope", builtin)
            .shared();
        let custom = custom.map(FutureExt::shared);
        self.load_workspace_overview(
            refresh,
            Some(SharedScopeReads {
                builtin: builtin.clone(),
                custom: custom.clone(),
            }),
            cx,
        );
        // As the web's keepPreviousData does, rows of the same context stay visible until the new read settles.
        if !self
            .rows
            .first()
            .is_some_and(|row| row.cluster == self.scope.context)
        {
            self.load = Load::Loading;
            self.rows = Rc::default();
            self.warnings.clear();
        }
        self.prepare_rows();
        self.filter(cx);
        self.custom_load = if custom.is_some() {
            CustomLoad::Loading
        } else {
            CustomLoad::Idle
        };
        // One task, so dropping it (a newer scope or refresh) cancels the custom read too.
        self.scope_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = builtin.await;
            let applied = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return false;
                }
                this.live.scope_loading = false;
                let ready = match result {
                    Ok(result) => {
                        this.namespaces = result.namespaces;
                        this.rows = Rc::new(crate::model::PreparedRows::new(result.resources));
                        this.warnings = result.warnings;
                        this.load = Load::Ready;
                        true
                    }
                    Err(error) => {
                        this.live.refresh_error = Some(error.message.clone());
                        this.load = Load::Failed(error.message);
                        this.custom_load = CustomLoad::Idle;
                        this.rows = Rc::default();
                        this.warnings.clear();
                        false
                    }
                };
                this.prepare_rows();
                this.filter(cx);
                cx.notify();
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
                    this.apply_custom_resources(result, cx);
                }
            });
        }));
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
        self.map.replace_topology(map::TopologyLoad::Loading);
        let request = self.request.clone();
        let network = self.map.network;
        let backend = self.backend.clone();
        let scope = self.scope.clone();
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
                this.map.replace_topology(match result {
                    Ok(topology) => map::TopologyLoad::Ready(topology),
                    Err(error) => map::TopologyLoad::Failed(error),
                });
                this.rebuild_map();
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.live_menu_open = false;
        if self.scope.context.is_empty() {
            self.load_contexts(window, cx)
        } else {
            self.live.refreshing = true;
            self.live.scope_loading = true;
            self.live.refresh_error = None;
            let read = self.application_resources_read(true, cx);
            let read = self.timings.measure("list_resource_scope", read);
            let request = self.request.clone();
            self.scope_task = Some(cx.spawn_in(window, async move |this, cx| {
                let result = read.await;
                let _ = this.update(cx, |this, cx| {
                    if !Rc::ptr_eq(&this.request, &request) {
                        return;
                    }
                    this.live.refreshing = false;
                    this.live.scope_loading = false;
                    match result {
                        Ok(result) => {
                            this.rows = Rc::new(crate::model::PreparedRows::new(result.resources));
                            this.namespaces = result.namespaces;
                            this.warnings = result.warnings;
                            this.load = Load::Ready;
                            this.custom_load = CustomLoad::Ready;
                            this.prepare_rows();
                            this.filter(cx);
                        }
                        Err(error) => this.live.refresh_error = Some(error.message),
                    }
                    cx.notify();
                });
            }));
            self.load_topology(cx);
            self.load_inventory(true, cx);
            self.load_page(cx);
        }
        cx.notify();
    }

    pub(super) fn filter(&mut self, cx: &mut Context<Self>) {
        self.update_saved_overview();
        self.filter_rows(cx);
        self.rebuild_map();
        cx.notify();
    }

    /// Resolves cluster-row inheritance before the filter source joins it with scope rows.
    pub(super) fn prepare_rows(&mut self) {
        self.cluster.prepared = crate::model::inherit_gitops_owners(&self.cluster.rows);
    }

    /// The resource scope read in two stages: built-ins first, then custom kinds.
    /// A focused GitOps application keeps its single application read.
    fn scope_reads(
        &self,
        mut refresh: bool,
        cx: &Context<Self>,
    ) -> (
        crate::backend::Read<ScopeResult>,
        Option<crate::backend::Read<ScopeResult>>,
    ) {
        if self.focused_application().is_some() {
            return (self.application_resources_read(refresh, cx), None);
        }
        let backend = self.backend.clone();
        let scope = self.scope.clone();
        let custom = self
            .backend
            .scope_custom_resources(scope.clone())
            .map(|first| {
                let backend = self.backend.clone();
                let scope = scope.clone();
                let mut first = Some(first);
                crate::read_retry::read(cx.background_executor().clone(), move || {
                    first.take().unwrap_or_else(|| {
                        backend
                            .scope_custom_resources(scope.clone())
                            .expect("custom read remains available")
                    })
                })
            });
        let builtin = crate::read_retry::read(cx.background_executor().clone(), move || {
            backend.scope_builtin_resources(scope.clone(), std::mem::take(&mut refresh))
        });
        (builtin, custom)
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
