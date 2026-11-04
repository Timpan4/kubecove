//! Warms the palette's index through the same typed reads as the resource browser.
//! The web uses `buildGlobalSearchFetchKeys`; native discovery remains limited to
//! the kinds its backend supports.

use super::Inspector;
use crate::backend::{CLUSTER_KINDS, Scope};
use crate::model::WORKSPACE_KINDS;
use crate::palette;
use gpui_kit::{Context, SharedString, Task};
use kubecove_lib::models::ResourceSummary;
use std::{collections::HashSet, rc::Rc};

#[derive(Default)]
enum Status {
    #[default]
    Idle,
    Loading,
    Ready,
    Partial(SharedString),
}

#[derive(Default)]
pub(super) struct SearchRead {
    task: Option<Task<()>>,
    request: Rc<()>,
    status: Status,
}

impl SearchRead {
    pub(super) fn loading(&self) -> bool {
        matches!(self.status, Status::Loading)
    }

    pub(super) fn message(&self) -> Option<SharedString> {
        match &self.status {
            Status::Loading => Some("Loading resources...".into()),
            Status::Partial(warning) => Some(warning.clone()),
            Status::Idle | Status::Ready => None,
        }
    }
}

impl Inspector {
    /// Reads the whole workspace scope, even when the resource browser is
    /// currently filtered to a namespace or its cluster page has not opened.
    pub(super) fn warm_palette(&mut self, cx: &mut Context<Self>) {
        let context = self.scope.context.clone();
        let workspace = self.backend.resources(
            Scope {
                context: context.clone(),
                namespace: None,
            },
            false,
        );
        let cluster = self.backend.cluster_resources(context.clone(), false);
        let state = &mut self.palette.search;
        state.status = Status::Loading;
        state.request = Rc::new(());
        let request = state.request.clone();
        state.task = Some(cx.spawn(async move |this, cx| {
            // NativeBackend starts both host tasks when the reads are created.
            // Dropping this task drops both reads and aborts their host tasks.
            let results = [workspace.await, cluster.await];
            let _ = this.update(cx, |this, cx| {
                if !this.palette.open
                    || this.scope.context != context
                    || !Rc::ptr_eq(&this.palette.search.request, &request)
                {
                    return;
                }
                let mut rows = Vec::new();
                let mut warnings = Vec::new();
                for result in results {
                    match result {
                        Ok(result) => {
                            rows.extend(result.resources);
                            warnings.extend(result.warnings);
                        }
                        Err(error) => warnings.push(error.message),
                    }
                }
                this.palette.search.status = if warnings.is_empty() {
                    Status::Ready
                } else {
                    Status::Partial(
                        format!("Resource search is partial: {}", warnings.join("; ")).into(),
                    )
                };
                this.set_palette_resources(rows);
                this.refresh_palette(cx);
            });
        }));
        cx.notify();
    }

    fn set_palette_resources(&mut self, rows: Vec<ResourceSummary>) {
        let mut identities = HashSet::new();
        let mut resources: Vec<_> = rows
            .into_iter()
            .filter(|row| self.launcher.in_scope(row))
            .filter(|row| {
                identities.insert((
                    row.cluster.clone(),
                    row.group.clone(),
                    row.kind.clone(),
                    row.namespace.clone(),
                    row.name.clone(),
                ))
            })
            .collect();
        // Keep API order within each kind, matching the web's fetch-key order.
        resources.sort_by_key(|row| {
            WORKSPACE_KINDS
                .iter()
                .chain(CLUSTER_KINDS)
                .position(|kind| *kind == row.kind)
                .unwrap_or(WORKSPACE_KINDS.len() + CLUSTER_KINDS.len())
        });
        self.palette.index = resources.iter().map(palette::search_text).collect();
        self.palette.resources = resources;
    }
}
