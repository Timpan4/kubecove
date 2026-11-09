//! Cluster Overview: the resource browser over the cluster-scoped kinds that
//! the web opens for the `clusterOverview` sidebar section (`resolveTreeScope`
//! in `src/lib/tree-nav.ts`). Like `resourceBrowserAvailableKinds` in
//! `workspaceShellModel.ts`, present custom resource kinds join the scope.

use super::{Inspector, Load, Page};
use crate::backend::CLUSTER_KINDS;
use gpui_kit::{Context, Task, Window};
use kubecove_lib::models::ResourceSummary;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct ClusterState {
    /// Replaced with the inventory before starting a new cluster read.
    pub(super) request: Rc<()>,
    /// The context `rows` belong to.
    context: String,
    /// `None` until the page first opens for `context`.
    load: Option<Load>,
    pub(super) rows: Vec<ResourceSummary>,
    /// `rows` with inherited GitOps owners, rebuilt by `prepare_rows`.
    pub(super) prepared: Vec<ResourceSummary>,
    /// The list version of each kind in `rows`, by watch signature.
    pub(super) versions: crate::backend::ListVersions,
    warnings: Vec<String>,
    task: Option<Task<()>>,
}

impl Inspector {
    pub(super) fn open_cluster_overview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page == Page::ClusterOverview || !self.can_leave(cx) {
            return;
        }
        self.close_detail(cx);
        self.set_page(Page::ClusterOverview);
        self.table_page = 0;
        // The section has no namespace; the web scope falls back to the
        // workspace namespaces, which are all namespaces here.
        if self.scope.namespace.take().is_some() {
            self.load_scope(false, window, cx);
        } else {
            self.load_cluster(false, cx);
            self.filter(cx);
        }
    }

    /// Called by `load_scope`: re-reads the inventories for a new context or
    /// a refresh. Cluster rows load for the overview or selected cluster kinds.
    pub(super) fn load_inventory(&mut self, refresh: bool, cx: &mut Context<Self>) {
        self.load_gitops(refresh, cx);
        if self.wants_cluster_rows() {
            self.load_cluster(refresh, cx);
        }
    }

    pub(super) fn load_cluster(&mut self, mut refresh: bool, cx: &mut Context<Self>) {
        let current = self.cluster.context == self.scope.context;
        if current && !refresh && self.cluster.load.is_some() {
            return;
        }
        // Replacing the state drops, and so cancels, an older read.
        self.cluster = ClusterState {
            context: self.scope.context.clone(),
            load: Some(Load::Loading),
            ..ClusterState::default()
        };
        let backend = self.backend.clone();
        let context = self.scope.context.clone();
        let read = crate::read_retry::read(cx.background_executor().clone(), move || {
            backend.cluster_resources(context.clone(), std::mem::take(&mut refresh))
        });
        self.cluster.task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(result) => {
                        this.cluster.rows = result.resources;
                        this.cluster.versions = result.versions;
                        this.cluster.warnings = result.warnings;
                        this.cluster.load = Some(Load::Ready);
                    }
                    Err(error) => this.cluster.load = Some(Load::Failed(error.message)),
                }
                this.prepare_rows();
                this.filter(cx);
            });
        }));
    }

    /// Kinds the page lists before present custom kinds are added.
    pub(super) fn base_kinds(&self) -> &'static [&'static str] {
        if self.page == Page::ClusterOverview {
            CLUSTER_KINDS
        } else if self.application_workspace_name().is_some() {
            crate::backend::APPLICATION_KINDS
        } else {
            crate::model::WORKSPACE_KINDS
        }
    }

    /// Cluster rows join the filter source when the selected scope needs them.
    pub(super) fn cluster_rows(&self) -> &[ResourceSummary] {
        if self.wants_cluster_rows() {
            &self.cluster.rows
        } else {
            &[]
        }
    }

    /// The prepared cluster rows that join the filter source.
    pub(super) fn cluster_prepared(&self) -> &[ResourceSummary] {
        if self.wants_cluster_rows() {
            &self.cluster.prepared
        } else {
            &[]
        }
    }

    /// Whether a cluster-row read is still loading, so a cluster kind's watch
    /// waits for the version of the rows it will show.
    pub(super) fn cluster_loading(&self) -> bool {
        matches!(self.cluster.load, Some(Load::Loading))
    }

    fn wants_cluster_rows(&self) -> bool {
        self.page == Page::ClusterOverview
            || self
                .kind_selection
                .chosen
                .as_ref()
                .is_some_and(|chosen| CLUSTER_KINDS.iter().any(|kind| chosen.contains(*kind)))
    }

    /// The resource browser's load state: the workspace read, then the
    /// cluster read on the Cluster Overview.
    pub(super) fn page_load(&self) -> &Load {
        match (&self.scope_load.rows, &self.cluster.load) {
            (Load::Ready, Some(load)) if self.page == Page::ClusterOverview => load,
            (load, _) => load,
        }
    }

    pub(super) fn page_warnings(&self) -> impl Iterator<Item = &String> {
        let cluster: &[String] = if self.page == Page::ClusterOverview {
            &self.cluster.warnings
        } else {
            &[]
        };
        self.warnings.iter().chain(cluster)
    }
}
