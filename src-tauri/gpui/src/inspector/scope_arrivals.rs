//! `load_scope`'s read applied one context at a time, as each context
//! answers, so a slow or unreachable context never holds back the rows of
//! the contexts that answered.
use super::Inspector;
use super::load_trace::LoadKind;
use super::resource_observation::rank_order;
use crate::backend::ScopeResult;
use crate::model::PreparedRows;
use gpui_kit::{BackgroundExecutor, Context};
use kubecove_lib::models::{AppError, ResourceSummary};
use std::{rc::Rc, time::Instant};

/// A context's rows joined with the rows of the contexts that already
/// answered, in context rank order, and prepared on the background executor
/// so applying them on the UI thread only swaps them in.
pub(super) async fn prepare_arrival(
    executor: &BackgroundExecutor,
    contexts: Vec<String>,
    mut rows: Vec<ResourceSummary>,
    mut result: Result<ScopeResult, AppError>,
) -> (Result<ScopeResult, AppError>, PreparedRows) {
    executor
        .spawn(async move {
            if let Ok(result) = &mut result {
                rows.append(&mut result.resources);
            }
            rank_order(&mut rows, &contexts);
            let rows = PreparedRows::with_inherited_owners(rows, &contexts);
            (result, rows)
        })
        .await
}

impl Inspector {
    /// The page's warnings: the scope read's `read` warnings, then kind
    /// discovery's, which may have settled before the read applied.
    pub(super) fn scope_warnings(&self, mut read: Vec<String>) -> Vec<String> {
        read.extend(self.live.discovery_warnings.iter().cloned());
        read
    }

    /// The rows of every context but `context`, which its arrival replaces,
    /// or `None` when a newer request replaced `request`. Watch patches wait
    /// until the arrival applies, so none is lost to the rows being prepared.
    pub(super) fn begin_arrival(
        &mut self,
        request: &Rc<()>,
        context: &str,
    ) -> Option<Vec<ResourceSummary>> {
        if !Rc::ptr_eq(&self.request, request) {
            return None;
        }
        Some(self.observation.begin_arrival(context))
    }

    /// Applies one context's built-in read with the rows `prepare_arrival`
    /// built. Once every context answered and no context's rows show, the load
    /// fails with every context's error. Returns false when a newer request replaced `request`.
    pub(super) fn apply_arrival(
        &mut self,
        request: &Rc<()>,
        context: &str,
        result: Result<ScopeResult, AppError>,
        rows: PreparedRows,
        cx: &mut Context<Self>,
    ) -> bool {
        if !Rc::ptr_eq(&self.request, request) {
            return false;
        }
        let apply_started = Instant::now();
        let applied = self.observation.install_arrival(context, result, rows);
        if let Some(namespaces) = applied.namespaces {
            self.namespaces = namespaces;
        }
        if let Some(error) = applied.error {
            self.live.refresh_error = Some(error);
            self.warnings.clear();
        } else {
            self.warnings = self.scope_warnings(self.observation.warnings());
        }
        self.prepare_rows();
        self.filter(cx);
        cx.notify();
        self.note_apply(
            LoadKind::Scope,
            apply_started.elapsed(),
            self.observation.settled(),
        );
        true
    }

    /// Applies one context's custom-kind read, which follows its built-ins.
    pub(super) fn apply_custom_arrival(
        &mut self,
        request: &Rc<()>,
        context: &str,
        result: Result<ScopeResult, AppError>,
        cx: &mut Context<Self>,
    ) {
        if !Rc::ptr_eq(&self.request, request) {
            return;
        }
        let apply_started = Instant::now();
        self.apply_custom_resources(context, result, cx);
        self.note_apply(
            LoadKind::Scope,
            apply_started.elapsed(),
            self.observation.settled(),
        );
    }
}
