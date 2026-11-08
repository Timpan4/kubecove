//! Workspace context ownership for Overview reads, separate from the resource browser.
use super::Inspector;
use super::scope::SharedScopeReads;
use crate::backend::{Read, Scope};
use gpui_kit::{Context, Task};
use std::rc::Rc;

pub(super) struct OverviewRead {
    pub rows: Vec<kubecove_lib::models::ResourceSummary>,
    /// The lists behind `rows`, so entry reconciliation drops only what a list missed.
    pub listed: Vec<crate::backend::Listed>,
    pub load: super::Load,
    pub warnings: Vec<String>,
    pub retried: bool,
    _task: Task<()>,
}

impl Inspector {
    pub(super) fn retry_overview(&mut self, cx: &mut Context<Self>) {
        let rows = self.workspace_rows().cloned().collect();
        let request = self.request.clone();
        let read = self.workspace_resources_read(true, cx);
        let task = cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return;
                }
                let Some(state) = &mut this.workspace_read else {
                    return;
                };
                match result {
                    Ok(result) => {
                        state.rows = result.resources;
                        state.listed = result.listed;
                        state.warnings = result.warnings;
                        state.load = super::Load::Ready;
                        state.retried = true;
                    }
                    Err(error) => state.load = super::Load::Failed(error.message),
                }
                this.reconcile_workspace_entries(cx);
                this.update_saved_overview();
                cx.notify();
            });
        });
        self.workspace_read = Some(OverviewRead {
            rows,
            listed: vec![],
            warnings: vec![],
            load: super::Load::Loading,
            retried: false,
            _task: task,
        });
        cx.notify();
    }
    /// True when the Overview read is the same request as the scope read: one
    /// saved context, the current context, no namespace and no focused Application.
    fn overview_matches_scope(&self) -> bool {
        let contexts = self.workspace_contexts();
        self.focused_application().is_none()
            && self.scope.namespace.is_none()
            && matches!(contexts.as_slice(), [only] if *only == self.scope.context)
    }

    pub(super) fn workspace_contexts(&self) -> Vec<String> {
        self.launcher
            .active
            .and_then(|id| self.launcher.saved.iter().find(|saved| saved.id == id))
            .filter(|saved| saved.context == self.scope.context)
            .map_or_else(
                || vec![self.scope.context.clone()],
                |saved| saved.contexts().into_iter().map(str::to_owned).collect(),
            )
    }

    pub(super) fn workspace_resources_read(
        &self,
        refresh: bool,
        cx: &Context<Self>,
    ) -> Read<crate::backend::ScopeResult> {
        let reads = self
            .workspace_contexts()
            .into_iter()
            .map(|context| {
                let mut refresh = refresh;
                let backend = self.backend.clone();
                let scope = Scope {
                    context: context.clone(),
                    namespace: None,
                };
                let read = crate::read_retry::read(cx.background_executor().clone(), move || {
                    backend.resources(scope.clone(), std::mem::take(&mut refresh))
                });
                (context, read)
            })
            .collect();
        crate::workspaces::reads::resources(reads)
    }

    /// Every Overview visit reads the saved scope, as the web's workspace query
    /// does, so restore status and entry reconciliation never reuse a stale read.
    /// `shared` is the resource scope read that starts in the same pass. When the
    /// Overview requests that exact scope, it joins that read instead of issuing its own.
    pub(super) fn load_workspace_overview(
        &mut self,
        refresh: bool,
        shared: Option<SharedScopeReads>,
        cx: &mut Context<Self>,
    ) {
        self.workspace_read = None;
        if self.page != super::Page::Overview {
            self.update_saved_overview();
            return;
        }
        self.load_restore_inventory(cx);
        let request = self.request.clone();
        let read = match shared {
            Some(shared) if self.overview_matches_scope() => shared.overview_read(),
            _ => self.workspace_resources_read(refresh, cx),
        };
        let task = cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &request) {
                    return;
                }
                let Some(state) = &mut this.workspace_read else {
                    return;
                };
                match result {
                    Ok(result) => {
                        state.rows = result.resources;
                        state.listed = result.listed;
                        state.warnings = result.warnings;
                        state.load = super::Load::Ready;
                    }
                    Err(error) => state.load = super::Load::Failed(error.message),
                }
                this.reconcile_workspace_entries(cx);
                this.update_saved_overview();
                cx.notify();
            });
        });
        self.workspace_read = Some(OverviewRead {
            retried: false,
            rows: vec![],
            listed: vec![],
            warnings: vec![],
            load: super::Load::Loading,
            _task: task,
        });
    }

    pub(super) fn overview_load(&self) -> &super::Load {
        self.workspace_read
            .as_ref()
            .map_or(&self.load, |state| &state.load)
    }
}
