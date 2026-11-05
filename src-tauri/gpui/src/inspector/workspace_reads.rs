//! Workspace context ownership for Overview reads, separate from the resource browser.
use super::Inspector;
use crate::backend::{Read, Scope};
use gpui_kit::{Context, Task};
use std::rc::Rc;

pub(super) struct OverviewRead {
    pub rows: Vec<kubecove_lib::models::ResourceSummary>,
    pub load: super::Load,
    pub warnings: Vec<String>,
    _task: Task<()>,
}

impl Inspector {
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
    ) -> Read<crate::backend::ScopeResult> {
        let reads = self
            .workspace_contexts()
            .into_iter()
            .map(|context| {
                let read = self.backend.resources(
                    Scope {
                        context: context.clone(),
                        namespace: None,
                    },
                    refresh,
                );
                (context, read)
            })
            .collect();
        crate::workspaces::reads::resources(reads)
    }

    pub(super) fn load_workspace_overview(&mut self, refresh: bool, cx: &mut Context<Self>) {
        self.workspace_read = None;
        if self.page != super::Page::Overview
            || (self.workspace_contexts().len() <= 1 && self.scope.namespace.is_none())
        {
            return;
        }
        let request = self.request.clone();
        let read = self.workspace_resources_read(refresh);
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
                        state.warnings = result.warnings;
                        state.load = super::Load::Ready;
                    }
                    Err(error) => state.load = super::Load::Failed(error.message),
                }
                cx.notify();
            });
        });
        self.workspace_read = Some(OverviewRead {
            rows: vec![],
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
