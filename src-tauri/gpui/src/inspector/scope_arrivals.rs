//! `load_scope`'s read applied one context at a time, as each context
//! answers, so a slow or unreachable context never holds back the rows of
//! the contexts that answered.
use super::Inspector;
use super::load_trace::LoadKind;
use super::scope_load::{all_failed, rank_order};
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
        self.scope_load.set_preparing(true);
        if self.scope_load.contexts().len() < 2 {
            return Some(Vec::new());
        }
        let mut rows = self.rows.backend_rows();
        rows.retain(|row| row.cluster != context);
        Some(rows)
    }

    /// Applies one context's built-in read with the rows `prepare_arrival`
    /// built. Once every context answered and none succeeded, the load fails
    /// with every context's error. Returns false when a newer request replaced `request`.
    pub(super) fn apply_arrival(
        &mut self,
        request: &Rc<()>,
        context: &str,
        result: Result<ScopeResult, AppError>,
        rows: PreparedRows,
        failures: &mut Vec<(String, AppError)>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !Rc::ptr_eq(&self.request, request) {
            return false;
        }
        let apply_started = Instant::now();
        self.scope_load.set_preparing(false);
        // The context's watches start at the versions of the rows it read.
        self.live
            .starts
            .retain(|id, _| !crate::backend::watch_id_in(context, id));
        match result {
            Ok(result) => self.scope_load.context_settled(context, Ok(result)),
            Err(error) => {
                self.scope_load.context_settled(context, Err(&error));
                failures.push((context.to_owned(), error));
            }
        }
        // The picker lists the namespaces of every context whose rows show,
        // kept ones included, so a slow context's namespaces stay with its rows.
        if let Some(namespaces) = self.scope_load.shown_namespaces() {
            self.namespaces = namespaces;
        }
        if !self.scope_load.loading() && !self.scope_load.any_ready() {
            let contexts = self.scope_load.contexts();
            failures.sort_by_key(|(context, _)| contexts.iter().position(|other| other == context));
            let error = all_failed(
                failures
                    .drain(..)
                    .map(|(context, error)| (context, Err(error)))
                    .collect(),
            );
            self.live.refresh_error = Some(error.message.clone());
            self.scope_load.all_failed(error.message);
            self.rows = Rc::default();
            self.warnings.clear();
        } else {
            self.rows = Rc::new(rows);
            self.warnings = self.scope_warnings(self.scope_load.warnings());
        }
        self.prepare_rows();
        self.filter(cx);
        cx.notify();
        self.note_apply(
            LoadKind::Scope,
            apply_started.elapsed(),
            self.scope_load.settled(),
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
            self.scope_load.settled(),
        );
    }
}
