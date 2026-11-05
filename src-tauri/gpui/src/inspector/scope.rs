//! Context and resource-scope reads, refresh, and visible resource filtering.

use super::{Inspector, Load, map};
use gpui_kit::component::select::SearchableVec;
use gpui_kit::{Context, SharedString, Window};
use kubecove_lib::models::ResourceSummary;
use std::rc::Rc;

impl Inspector {
    pub(super) fn load_contexts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.context_task.take();
        self.load = Load::Loading;
        let read = self.backend.contexts();
        let read = self.timings.measure("list_kube_contexts", read);
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
        self.load_workspace_overview(refresh, cx);
        self.load = Load::Loading;
        self.rows.clear();
        self.warnings.clear();
        self.filter(cx);
        let read = self.application_resources_read(refresh);
        let read = self.timings.measure("list_resource_scope", read);
        self.scope_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return;
                }
                this.live.scope_loading = false;
                match result {
                    Ok(result) => {
                        this.namespaces = result.namespaces;
                        this.rows = result.resources;
                        this.warnings = result.warnings;
                        this.load = Load::Ready;
                    }
                    Err(error) => {
                        this.live.refresh_error = Some(error.message.clone());
                        this.load = Load::Failed(error.message);
                    }
                }
                this.filter(cx);
                cx.notify();
            });
        }));
        // The map loads on its own, like the web's topology query, so a
        // topology failure leaves the table usable.
        self.load_metrics(window, cx);
        self.load_topology(window, cx);
        self.load_inventory(refresh, cx);
        self.load_page(cx);
        cx.notify();
    }

    pub(super) fn set_map_mode(
        &mut self,
        network: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.map.network == network {
            return;
        }
        self.map.network = network;
        self.load_topology(window, cx);
    }

    fn load_topology(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.map.cache.take();
        self.map.topology = map::TopologyLoad::Loading;
        let request = self.request.clone();
        let network = self.map.network;
        let read = if network {
            self.backend.network_topology(self.scope.clone())
        } else {
            self.backend.topology(self.scope.clone())
        };
        let read = self.timings.measure("list_resource_topology", read);
        self.topology_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) || this.map.network != network {
                    return;
                }
                this.map.topology = match result {
                    Ok(topology) => map::TopologyLoad::Ready(topology),
                    Err(error) => map::TopologyLoad::Failed(error.message),
                };
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
            let read = self.application_resources_read(true);
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
                            this.rows = result.resources;
                            this.namespaces = result.namespaces;
                            this.warnings = result.warnings;
                            this.load = Load::Ready;
                            this.filter(cx);
                        }
                        Err(error) => this.live.refresh_error = Some(error.message),
                    }
                    cx.notify();
                });
            }));
            self.load_inventory(true, cx);
            self.load_page(cx);
        }
        cx.notify();
    }

    pub(super) fn filter(&mut self, cx: &mut Context<Self>) {
        let search = self.search.read(cx).value().trim().to_lowercase();
        let kinds = self.scope_kinds();
        self.visible = self
            .rows
            .iter()
            .chain(self.cluster_rows())
            .filter(|row| self.row_visible(row, &search, &kinds))
            .cloned()
            .collect();
        self.rebuild_map();
        cx.notify();
    }

    /// Rows in the saved workspace kinds, which the overview summarizes.
    pub(super) fn workspace_rows(&self) -> impl Iterator<Item = &ResourceSummary> {
        self.workspace_read
            .as_ref()
            .map_or(&self.rows, |state| &state.rows)
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
        for row in &self.rows {
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
