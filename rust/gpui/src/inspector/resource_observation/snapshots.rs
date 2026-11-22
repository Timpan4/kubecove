//! Installs complete context evidence and its versions in one operation.
use super::{Applied, ResourceObservation};
use crate::backend::ScopeResult;
use crate::inspector::Load;
use crate::model::{ObservedNames, PreparedRows, RowsChange};
use crate::workspaces::reads::ContextResults;
use kubecove_lib::models::{AppError, NamespaceSummary, ResourceSummary};
use std::{rc::Rc, time::Instant};

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
fn all_failed(results: &ContextResults<ScopeResult>) -> AppError {
    let several = results.len() > 1;
    let mut failures = results
        .iter()
        .filter_map(|(context, result)| result.as_ref().err().map(|error| (context, error)));
    let (first_context, first) = failures.next().expect("every context failed");
    let mut error = first.clone();
    if !several {
        return error;
    }
    let line = |context: &str, error: &AppError| {
        format!(
            "Context \"{context}\", operation \"resource discovery\": {}",
            error.message
        )
    };
    let mut lines = vec![line(first_context, &error)];
    lines.extend(failures.map(|(context, other)| line(context, other)));
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
        scope_started: Instant,
    ) -> Arrival {
        self.preparing = false;
        let was_shown = self.shows(context);
        self.starts
            .retain(|id, _| !crate::backend::watch_id_in(context, id));
        match result {
            Ok(result) => self.context_settled(context, Ok(result), scope_started),
            Err(error) => {
                self.context_settled(context, Err(&error), scope_started);
                self.failures.push((context.to_owned(), error));
            }
        }
        let namespaces = self.shown_namespaces().or_else(|| was_shown.then(Vec::new));
        let error = if !self.loading() && !self.any_shown() {
            self.failures.sort_by_key(|(context, _)| {
                self.contexts.iter().position(|other| other == context)
            });
            let error = all_failed(
                &self
                    .failures
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
        scope_started: Instant,
    ) {
        let added = self.custom_settled(context, result, scope_started);
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
        ScopeRows { results, rows }: ScopeRows,
        scope_started: Instant,
    ) -> Result<Applied, String> {
        self.starts.clear();
        if !results.is_empty() && results.iter().all(|(_, result)| result.is_err()) {
            let error = all_failed(&results).message;
            let several = results.len() > 1;
            for (context, result) in results {
                if let (Some(index), Err(error)) = (self.position(&context), result) {
                    self.parts[index].refresh_error =
                        Some(context_message(several, &context, &error.message));
                }
            }
            self.refresh_settled(Err(error.clone()));
            return Err(error);
        }
        let span = crate::frame_profile::scope("scope", "apply_parts");
        let applied = self.apply_parts(results, true, scope_started);
        drop(span);
        let span = crate::frame_profile::scope("scope", "swap_rows");
        self.data = Rc::new(self.keep_failed_contexts(rows, &applied.kept));
        drop(span);
        self.refresh_settled(Ok(()));
        Ok(applied)
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
        added: &mut Vec<crate::model::Added>,
    ) -> (Vec<ResourceSummary>, RowsChange) {
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
            .patch_noting(&scope.context, key, rows, names, added);
        self.rows_patched();
        retired
    }

    pub(in crate::inspector) fn rows_patched(&mut self) {
        if !matches!(self.status, Load::Ready) && !self.data.is_empty() {
            self.status = Load::Ready;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ResourceObservation, ScopeResult, ScopeRows};
    use crate::backend::{Listed, watch_id};
    use kubecove_lib::models::{WatchResourceKey, WatchResourceKind};
    use std::time::{Duration, Instant};

    #[test]
    fn scope_refresh_clears_only_kind_failures_it_supersedes() {
        let context = "development";
        let other = "production";
        let key = WatchResourceKey {
            resource_kind: WatchResourceKind {
                kind: "Pod".into(),
                group: None,
                version: None,
                api_version: None,
                plural: None,
                namespaced: None,
            },
            namespace: None,
        };
        let id = watch_id(context, &key);
        let other_id = watch_id(other, &key);
        let mut observation = ResourceObservation {
            contexts: vec![context.into(), other.into()],
            parts: vec![Default::default(), Default::default()],
            ..Default::default()
        };
        let older = Instant::now();
        let kind_started = older + Duration::from_nanos(1);
        let newer = kind_started + Duration::from_nanos(1);
        observation.record_kind_error(context, &key, "Kind list failed".into(), kind_started);
        observation.record_kind_error(other, &key, "Other list failed".into(), kind_started);
        let snapshot = || ScopeRows {
            results: vec![(
                context.into(),
                Ok(ScopeResult {
                    listed: vec![Listed {
                        context: context.into(),
                        key: key.clone(),
                    }],
                    ..Default::default()
                }),
            )],
            rows: Default::default(),
        };

        assert!(observation.install_refresh(snapshot(), older).is_ok());
        assert!(observation.kind_read_failed(context, &id));
        assert_eq!(observation.shown_refresh_error(), Some("Kind list failed"));
        assert!(observation.install_refresh(snapshot(), newer).is_ok());
        assert!(!observation.kind_read_failed(context, &id));
        assert_eq!(observation.shown_refresh_error(), None);
        assert!(observation.kind_read_failed(other, &other_id));
    }
}
