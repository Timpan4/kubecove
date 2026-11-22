//! Warms the palette's index through the same typed reads as the resource browser.
//! The web uses `buildGlobalSearchFetchKeys`; native discovery remains limited to
//! the kinds its backend supports.

use super::super::request::InFlight;
use super::Inspector;
use crate::backend::{CLUSTER_KINDS, Scope};
use crate::model::WORKSPACE_KINDS;
use crate::palette;
use gpui_kit::{Context, SharedString};
use kubecove_lib::models::ResourceSummary;
use std::collections::HashSet;

/// The read that warms the open palette's index. `Loading` owns the read, so
/// a new warm-up or closing the palette cancels it.
#[derive(Default)]
pub(super) enum SearchRead {
    #[default]
    Idle,
    Loading {
        _read: InFlight,
    },
    Ready,
    Partial(SharedString),
}

impl SearchRead {
    pub(super) fn loading(&self) -> bool {
        matches!(self, SearchRead::Loading { .. })
    }

    pub(super) fn message(&self) -> Option<SharedString> {
        match self {
            SearchRead::Loading { .. } => Some("Loading resources...".into()),
            SearchRead::Partial(warning) => Some(warning.clone()),
            SearchRead::Idle | SearchRead::Ready => None,
        }
    }
}

impl Inspector {
    /// Reads the whole workspace scope in every workspace context, even when the
    /// resource browser is currently filtered to a namespace or its cluster page
    /// has not opened. Cluster rows come from the browser's context, as in the
    /// snapshot the palette opened with.
    pub(super) fn warm_palette(&mut self, cx: &mut Context<Self>) {
        let context = self.scope.context.clone();
        let contexts = self.workspace_contexts();
        let several = contexts.len() > 1;
        let reads = contexts
            .into_iter()
            .map(|read_context| {
                let backend = self.backend.clone();
                let scope = Scope {
                    context: read_context.clone(),
                    namespace: None,
                };
                let read = crate::read_retry::read(cx.background_executor().clone(), move || {
                    backend.resources(scope.clone(), false)
                });
                (read_context, read)
            })
            .collect();
        let workspace = crate::workspaces::reads::settle(reads);
        let backend = self.backend.clone();
        let read_context = context.clone();
        let cluster = crate::read_retry::read(cx.background_executor().clone(), move || {
            backend.cluster_resources(read_context.clone(), false)
        });
        let read = cx.spawn(async move |this, cx| {
            // NativeBackend starts the host tasks when the reads are created.
            // Dropping this task drops the reads and aborts their host tasks.
            let (workspace, cluster) = (workspace.await, cluster.await);
            let _ = this.update(cx, |this, cx| {
                if this.scope.context != context {
                    return;
                }
                let mut rows = Vec::new();
                let mut warnings = Vec::new();
                let named = |read_context: &str, warning: String| {
                    if several {
                        format!("{read_context}: {warning}")
                    } else {
                        warning
                    }
                };
                // A failed read keeps the rows the palette opened with, so a
                // degraded backend does not take searchable resources away.
                let mut failed = Vec::new();
                for (read_context, result) in workspace {
                    match result {
                        Ok(result) => {
                            rows.extend(result.resources);
                            warnings.extend(
                                result
                                    .warnings
                                    .into_iter()
                                    .map(|warning| named(&read_context, warning)),
                            );
                        }
                        Err(error) => {
                            warnings.push(named(&read_context, error.message));
                            failed.push(read_context);
                        }
                    }
                }
                let cluster_failed = match cluster {
                    Ok(result) => {
                        rows.extend(result.resources);
                        warnings.extend(result.warnings);
                        false
                    }
                    Err(error) => {
                        warnings.push(error.message);
                        true
                    }
                };
                rows.extend(
                    std::mem::take(&mut this.palette.resources)
                        .into_iter()
                        .filter(|row| {
                            if CLUSTER_KINDS.contains(&row.kind.as_str()) {
                                cluster_failed
                            } else {
                                failed.contains(&row.cluster)
                            }
                        }),
                );
                this.palette.search = if warnings.is_empty() {
                    SearchRead::Ready
                } else {
                    SearchRead::Partial(
                        format!("Resource search is partial: {}", warnings.join("; ")).into(),
                    )
                };
                this.set_palette_resources(rows);
                this.refresh_palette(cx);
            });
        });
        self.palette.search = SearchRead::Loading { _read: read.into() };
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
