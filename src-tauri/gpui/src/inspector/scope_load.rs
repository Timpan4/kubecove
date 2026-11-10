//! The resource scope read and what the table shows for it
//! (`state-machines.md` #5). The web keeps this in its scope and custom-kind
//! queries; here one owner holds the read so the table cannot wait on a read
//! that no longer runs.
use super::{
    Inspector, Load, custom_resources::CustomLoad, request::InFlight, watch_status::LiveStatus,
};
use crate::backend::{ListVersions, ScopeResult, context_versions};
use crate::model::PreparedRows;
use crate::workspaces::reads::ContextResults;
use gpui_kit::Task;
use kubecove_lib::models::{AppError, NamespaceSummary, ResourceSummary};

pub(super) struct ScopeLoad {
    /// What the table shows for the workspace contexts.
    pub(super) rows: Load,
    /// The last settled custom-kind read. `custom` adds the pending one.
    custom: CustomLoad,
    read: Option<ScopeRead>,
    /// The contexts the rows come from, in rank order (`SavedWorkspace::contexts`).
    contexts: Vec<String>,
    /// Each context's part of the read, by its position in `contexts`.
    parts: Vec<ContextRows>,
}

/// One workspace context's part of the scope read. A refresh that fails for
/// one context keeps that context's part and rows.
#[derive(Default)]
struct ContextRows {
    /// The context's built-in read succeeded, so the table shows its rows.
    ready: bool,
    /// The list version of each kind in the context's rows, by `watch_id`.
    versions: ListVersions,
    /// The read's warnings, named after the context when the page reads several.
    warnings: Vec<String>,
}

/// What applying every context's read changes outside `ScopeLoad`.
#[derive(Default)]
pub(super) struct Applied {
    /// The namespaces of every context that answered, once each by name.
    pub(super) namespaces: Vec<NamespaceSummary>,
    /// The page's warnings, in context rank order.
    pub(super) warnings: Vec<String>,
    /// Contexts whose refresh failed, so the table keeps their last rows.
    pub(super) kept: Vec<String>,
}

/// Whether two context lists hold the same contexts. Workspace contexts are unique.
pub(super) fn same_contexts(left: &[String], right: &[String]) -> bool {
    left.len() == right.len() && left.iter().all(|context| right.contains(context))
}

/// A message about one context, named after it when the page reads several.
pub(super) fn context_message(several: bool, context: &str, message: &str) -> String {
    if several {
        format!("Context \"{context}\": {message}")
    } else {
        message.to_owned()
    }
}

/// Orders rows by the rank of their context, keeping each context's own order.
pub(super) fn rank_order(rows: &mut [ResourceSummary], contexts: &[String]) {
    rows.sort_by_key(|row| {
        contexts
            .iter()
            .position(|context| *context == row.cluster)
            .unwrap_or(contexts.len())
    });
}

/// `namespaces` plus each previous namespace it lacks, so a context whose
/// refresh failed keeps its namespaces in the picker.
pub(super) fn union_namespaces(
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
pub(super) fn all_failed(results: ContextResults<ScopeResult>) -> AppError {
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

/// The scope read in flight. Dropping it cancels the read.
enum ScopeRead {
    /// The built-in read of `load_scope`. With `custom`, the same task then
    /// reads the custom kinds.
    Builtin { custom: bool, _task: InFlight },
    /// The built-ins applied; the task reads the custom kinds.
    Custom { _task: InFlight },
    /// The live menu's Refresh (clear cache): built-ins and custom kinds in one read.
    Refresh { _task: InFlight },
}

impl Default for ScopeLoad {
    fn default() -> Self {
        Self {
            rows: Load::Loading,
            custom: CustomLoad::Idle,
            read: None,
            contexts: Vec::new(),
            parts: Vec::new(),
        }
    }
}

impl ScopeLoad {
    /// Starts `load_scope`'s read of `contexts`, replacing and so cancelling
    /// any scope read. Each context's part starts empty.
    pub(super) fn start(&mut self, contexts: Vec<String>, custom: bool, task: Task<()>) {
        self.custom = CustomLoad::Idle;
        self.parts = contexts.iter().map(|_| ContextRows::default()).collect();
        self.contexts = contexts;
        self.read = Some(ScopeRead::Builtin {
            custom,
            _task: task.into(),
        });
    }

    /// Starts the live menu's refresh, replacing and so cancelling any scope read.
    pub(super) fn start_refresh(&mut self, task: Task<()>) {
        self.read = Some(ScopeRead::Refresh { _task: task.into() });
    }

    /// Drops the scope read, which cancels it.
    pub(super) fn cancel(&mut self) {
        self.read = None;
    }

    /// Applies the built-in read. After a success the task goes on to the
    /// custom kinds when it reads them.
    pub(super) fn builtin_settled(&mut self, result: Result<(), String>) {
        let read = self.read.take();
        match result {
            Ok(()) => {
                self.rows = Load::Ready;
                if let Some(ScopeRead::Builtin {
                    custom: true,
                    _task,
                }) = read
                {
                    self.read = Some(ScopeRead::Custom { _task });
                }
            }
            Err(error) => {
                self.rows = Load::Failed(error);
                self.custom = CustomLoad::Idle;
            }
        }
    }

    /// Applies each context's custom read to a context whose built-ins
    /// loaded. Returns the rows to add and the page's new warnings. The custom
    /// kinds fail only when no context read them.
    pub(super) fn custom_settled(
        &mut self,
        results: ContextResults<ScopeResult>,
    ) -> (Vec<ResourceSummary>, Vec<String>) {
        self.read = None;
        let several = self.contexts.len() > 1;
        let mut rows = Vec::new();
        let mut warnings = Vec::new();
        let mut ok = false;
        for (context, result) in results {
            let Some(part) = self
                .position(&context)
                .map(|index| &mut self.parts[index])
                .filter(|part| part.ready)
            else {
                continue;
            };
            match result {
                Ok(result) => {
                    ok = true;
                    rows.extend(result.resources);
                    part.versions
                        .extend(context_versions(&context, result.versions));
                    let named: Vec<_> = result
                        .warnings
                        .iter()
                        .map(|warning| context_message(several, &context, warning))
                        .collect();
                    part.warnings.extend(named.iter().cloned());
                    warnings.extend(named);
                }
                Err(error) => warnings.push(context_message(
                    several,
                    &context,
                    &format!("Custom Resources: {}", error.message),
                )),
            }
        }
        self.custom = if ok {
            CustomLoad::Ready
        } else {
            CustomLoad::Failed
        };
        (rows, warnings)
    }

    /// Applies each context's read, whose rows the caller took, to its part.
    /// A failed context shows no rows and one warning. In a `refresh`, a
    /// context that failed after a successful read keeps its part and rows.
    pub(super) fn apply_parts(
        &mut self,
        results: ContextResults<ScopeResult>,
        refresh: bool,
    ) -> Applied {
        let several = results.len() > 1;
        let mut applied = Applied::default();
        for (context, result) in results {
            let index = self.position(&context).unwrap_or_else(|| {
                self.contexts.push(context.clone());
                self.parts.push(ContextRows::default());
                self.parts.len() - 1
            });
            let part = &mut self.parts[index];
            match result {
                Ok(result) => {
                    *part = ContextRows {
                        ready: true,
                        versions: context_versions(&context, result.versions),
                        warnings: result
                            .warnings
                            .iter()
                            .map(|warning| context_message(several, &context, warning))
                            .collect(),
                    };
                    applied.warnings.extend(part.warnings.iter().cloned());
                    for namespace in result.namespaces {
                        if !applied
                            .namespaces
                            .iter()
                            .any(|other| other.name == namespace.name)
                        {
                            applied.namespaces.push(namespace);
                        }
                    }
                }
                Err(error) if refresh && part.ready => {
                    applied.warnings.push(format!(
                        "Context \"{context}\": {}. Showing the last loaded data.",
                        error.message
                    ));
                    applied.warnings.extend(part.warnings.iter().cloned());
                    applied.kept.push(context);
                }
                Err(error) => {
                    *part = ContextRows::default();
                    applied
                        .warnings
                        .push(format!("Context \"{context}\": {}", error.message));
                }
            }
        }
        applied
    }

    fn position(&self, context: &str) -> Option<usize> {
        self.contexts.iter().position(|other| other == context)
    }

    /// The contexts the rows come from, in rank order.
    pub(super) fn contexts(&self) -> &[String] {
        &self.contexts
    }

    pub(super) fn same_contexts(&self, contexts: &[String]) -> bool {
        same_contexts(&self.contexts, contexts)
    }

    pub(super) fn has_context(&self, context: &str) -> bool {
        self.position(context).is_some()
    }

    /// The list version of the rows a watch shows, by `watch_id`.
    pub(super) fn version(&self, id: &str) -> Option<&String> {
        self.parts.iter().find_map(|part| part.versions.get(id))
    }

    /// The list versions of `context`'s rows, which a watch patch moves.
    pub(super) fn versions_mut(&mut self, context: &str) -> Option<&mut ListVersions> {
        let index = self.position(context)?;
        Some(&mut self.parts[index].versions)
    }

    /// Applies the refresh. A failure keeps rows the table shows; without
    /// rows, as when Refresh is pressed during a first load, it fails the load.
    pub(super) fn refresh_settled(&mut self, result: Result<(), String>) {
        self.read = None;
        match result {
            Ok(()) => {
                self.rows = Load::Ready;
                self.custom = CustomLoad::Ready;
            }
            Err(_) if matches!(self.rows, Load::Ready) => {}
            Err(error) => self.rows = Load::Failed(error),
        }
    }

    /// A built-in read or a refresh runs, so the rows may still change.
    pub(super) fn loading(&self) -> bool {
        matches!(
            self.read,
            Some(ScopeRead::Builtin { .. } | ScopeRead::Refresh { .. })
        )
    }

    pub(super) fn refreshing(&self) -> bool {
        matches!(self.read, Some(ScopeRead::Refresh { .. }))
    }

    pub(super) fn custom(&self) -> CustomLoad {
        match self.read {
            Some(ScopeRead::Builtin { custom: true, .. } | ScopeRead::Custom { .. }) => {
                CustomLoad::Loading
            }
            _ => self.custom,
        }
    }
}

impl Inspector {
    /// The refreshed rows plus the last rows of each context whose refresh
    /// failed, in context rank order.
    pub(super) fn keep_failed_contexts(&self, rows: PreparedRows, kept: &[String]) -> PreparedRows {
        if kept.is_empty() {
            return rows;
        }
        let mut all = rows.backend_rows();
        all.extend(
            self.rows
                .backend_rows()
                .into_iter()
                .filter(|row| kept.contains(&row.cluster)),
        );
        let contexts = self.scope_load.contexts();
        rank_order(&mut all, contexts);
        PreparedRows::with_inherited_owners(all, contexts)
    }

    /// The resource view's live status, `Refreshing` while the live menu's refresh runs.
    pub(super) fn resource_live_status(&self) -> LiveStatus {
        if self.scope_load.refreshing() {
            LiveStatus::Refreshing
        } else {
            self.live.settled_status()
        }
    }

    /// Whether row activity may animate: every watch is live and no read is pending.
    /// Mirrors `updatesCurrent` in `ResourceBrowser.svelte`.
    pub(super) fn updates_current(&self) -> bool {
        !self.scope_load.loading() && self.live.settled_status() == LiveStatus::Live
    }
}
