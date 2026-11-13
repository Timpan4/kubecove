//! Installs complete context evidence and its versions in one operation.
use super::{Applied, ResourceObservation};
use crate::backend::ScopeResult;
use crate::inspector::Load;
use crate::model::{ObservedNames, PreparedRows};
use crate::workspaces::reads::ContextResults;
use kubecove_lib::models::{AppError, NamespaceSummary, ResourceSummary};
use std::rc::Rc;

/// Whether two context lists hold the same contexts. Workspace contexts are unique.
pub(in crate::inspector) fn same_contexts(left: &[String], right: &[String]) -> bool {
    left.len() == right.len() && left.iter().all(|context| right.contains(context))
}

/// A message about one context, named after it when the page reads several.
pub(in crate::inspector) fn context_message(several: bool, context: &str, message: &str) -> String {
    if several {
        format!("Context \"{context}\": {message}")
    } else {
        message.to_owned()
    }
}

/// Orders rows by the rank of their context, keeping each context's own order.
pub(in crate::inspector) fn rank_order(rows: &mut [ResourceSummary], contexts: &[String]) {
    rows.sort_by_key(|row| {
        contexts
            .iter()
            .position(|context| *context == row.cluster)
            .unwrap_or(contexts.len())
    });
}

/// `namespaces` plus each previous namespace it lacks, so a context whose
/// refresh failed keeps its namespaces in the picker.
pub(in crate::inspector) fn union_namespaces(
    mut namespaces: Vec<NamespaceSummary>,
    previous: &[NamespaceSummary],
) -> Vec<NamespaceSummary> {
    for namespace in previous {
        if !namespaces.iter().any(|other| other.name == namespace.name) {
            namespaces.push(namespace.clone());
        }
    }
    namespaces
}

/// The error of a read in which every context failed: one context's error as
/// it is, or one error naming each context, as the web's
/// `WorkspaceResourceLoadError` does.
pub(in crate::inspector) fn all_failed(results: ContextResults<ScopeResult>) -> AppError {
    let several = results.len() > 1;
    let mut failures = results
        .into_iter()
        .filter_map(|(context, result)| result.err().map(|error| (context, error)));
    let (first_context, mut error) = failures.next().expect("every context failed");
    if !several {
        return error;
    }
    let line = |context: &str, error: &AppError| {
        format!(
            "Context \"{context}\", operation \"resource discovery\": {}",
            error.message
        )
    };
    let mut lines = vec![line(&first_context, &error)];
    lines.extend(failures.map(|(context, other)| line(&context, &other)));
    error.message = lines.join("\n");
    error
}

/// Prepared rows and the per-context results that describe them.
pub(in crate::inspector) struct ScopeRows {
    pub results: ContextResults<ScopeResult>,
    pub rows: PreparedRows,
}

pub(in crate::inspector) struct Arrival {
    pub namespaces: Option<Vec<NamespaceSummary>>,
    pub error: Option<String>,
}

impl ResourceObservation {
    pub(in crate::inspector) fn rows(&self) -> &Rc<PreparedRows> {
        &self.data
    }

    pub(in crate::inspector) fn status(&self) -> &Load {
        &self.status
    }

    pub(in crate::inspector) fn loading_contexts(&mut self) {
        self.status = Load::Loading;
    }

    pub(in crate::inspector) fn context_error(&mut self, message: String) {
        self.status = Load::Failed(message);
    }

    /// Keeps the previous evidence only for the same contexts.
    pub(in crate::inspector) fn prepare_scope(
        &mut self,
        contexts: &[String],
        namespace: Option<String>,
    ) -> bool {
        self.namespace = namespace;
        let kept = self.same_contexts(contexts) && !self.data.is_empty();
        if !kept {
            self.status = Load::Loading;
            self.data = Rc::default();
        }
        kept
    }

    /// Drops the old scope's reads, start versions, and staged changes together.
    pub(in crate::inspector) fn reset_scope(&mut self) {
        self.cancel();
        self.starts.clear();
        self.start_tasks.clear();
        self.pending.clear();
    }

    pub(in crate::inspector) fn clear(&mut self) {
        self.reset_scope();
        self.data = Rc::default();
    }

    /// Holds patches while a replacement snapshot is prepared off thread.
    pub(in crate::inspector) fn begin_arrival(&mut self, context: &str) -> Vec<ResourceSummary> {
        self.preparing = true;
        if self.contexts.len() < 2 {
            return Vec::new();
        }
        let mut rows = self.data.backend_rows();
        rows.retain(|row| row.cluster != context);
        rows
    }

    pub(in crate::inspector) fn install_arrival(
        &mut self,
        context: &str,
        result: Result<ScopeResult, AppError>,
        rows: PreparedRows,
    ) -> Arrival {
        self.preparing = false;
        let was_shown = self.shows(context);
        self.starts
            .retain(|id, _| !crate::backend::watch_id_in(context, id));
        match result {
            Ok(result) => self.context_settled(context, Ok(result)),
            Err(error) => {
                self.context_settled(context, Err(&error));
                self.failures.push((context.to_owned(), error));
            }
        }
        let namespaces = self.shown_namespaces().or_else(|| was_shown.then(Vec::new));
        let error = if !self.loading() && !self.any_shown() {
            self.failures.sort_by_key(|(context, _)| {
                self.contexts.iter().position(|other| other == context)
            });
            let error = all_failed(
                self.failures
                    .drain(..)
                    .map(|(context, error)| (context, Err(error)))
                    .collect(),
            );
            self.all_failed(error.message.clone());
            self.data = Rc::default();
            Some(error.message)
        } else {
            self.data = Rc::new(rows);
            None
        };
        Arrival { namespaces, error }
    }

    pub(in crate::inspector) fn install_custom(
        &mut self,
        context: &str,
        result: Result<ScopeResult, AppError>,
    ) {
        let added = self.custom_settled(context, result);
        if !added.is_empty() {
            let mut rows = self.data.backend_rows();
            rows.extend(added);
            rank_order(&mut rows, &self.contexts);
            self.data = Rc::new(PreparedRows::with_inherited_owners(rows, &self.contexts));
        }
    }

    /// Retires initial watch starts even when refresh replaced their first list.
    pub(in crate::inspector) fn install_refresh(
        &mut self,
        result: Result<ScopeRows, AppError>,
    ) -> Result<Applied, String> {
        self.starts.clear();
        match result {
            Ok(ScopeRows { results, rows }) => {
                let span = crate::frame_profile::scope("scope", "apply_parts");
                let applied = self.apply_parts(results, true);
                drop(span);
                let span = crate::frame_profile::scope("scope", "swap_rows");
                self.data = Rc::new(self.keep_failed_contexts(rows, &applied.kept));
                drop(span);
                self.refresh_settled(Ok(()));
                Ok(applied)
            }
            Err(error) => {
                self.refresh_settled(Err(error.message.clone()));
                Err(error.message)
            }
        }
    }

    fn keep_failed_contexts(&self, rows: PreparedRows, kept: &[String]) -> PreparedRows {
        if kept.is_empty() {
            return rows;
        }
        let mut all = rows.backend_rows();
        all.extend(
            self.data
                .backend_rows()
                .into_iter()
                .filter(|row| kept.contains(&row.cluster)),
        );
        rank_order(&mut all, &self.contexts);
        PreparedRows::with_inherited_owners(all, &self.contexts)
    }

    /// Changes the rows and their list version together.
    pub(in crate::inspector) fn patch(
        &mut self,
        scope: &crate::backend::Scope,
        key: &kubecove_lib::models::WatchResourceKey,
        rows: Vec<ResourceSummary>,
        version: Option<String>,
        names: Option<ObservedNames>,
    ) -> Vec<ResourceSummary> {
        if let Some(index) = self.position(&scope.context) {
            let id = crate::backend::watch_id(&scope.context, key);
            match version {
                Some(version) => self.parts[index].versions.insert(id, version),
                None => self.parts[index].versions.remove(&id),
            };
        }
        if !rows.is_empty() {
            self.mark_shown(&scope.context);
        }
        let retired = Rc::get_mut(&mut self.data)
            .expect("visible source released before patching")
            .patch(&scope.context, key, rows, names);
        self.rows_patched();
        retired
    }

    pub(in crate::inspector) fn rows_patched(&mut self) {
        if !matches!(self.status, Load::Ready) && !self.data.is_empty() {
            self.status = Load::Ready;
        }
    }
}
