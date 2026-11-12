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
    /// A refresh replaced, and so cancelled, a custom-kind read. If the
    /// refresh fails, the custom kinds are reported as failed, not idle.
    custom_interrupted: bool,
    /// A context's custom-kind read in the current load succeeded.
    custom_ok: bool,
    /// A context's rows are being prepared off the UI thread, together with
    /// the rows of the contexts that already answered, so watch patches wait.
    preparing: bool,
    read: Option<ScopeRead>,
    /// The contexts the rows come from, in rank order (`SavedWorkspace::contexts`).
    contexts: Vec<String>,
    /// Each context's part of the read, by its position in `contexts`.
    parts: Vec<ContextRows>,
}

/// One workspace context's part of the scope read. Each context applies when
/// it answers, so a slow or unreachable context never holds back the others.
/// A refresh that fails for one context keeps that context's part and rows.
#[derive(Default)]
struct ContextRows {
    /// The context's built-in read succeeded, so the table shows its rows.
    ready: bool,
    /// The table shows rows of the context: read in this load, or kept from
    /// the previous load of the same contexts until this one answers.
    shown: bool,
    /// The context's built-in read in the current load runs.
    pending: bool,
    /// The context's custom-kind read in the current load runs, or waits for its built-ins.
    custom_pending: bool,
    /// The list version of each kind in the context's rows, by `watch_id`.
    versions: ListVersions,
    /// The read's warnings, named after the context when the page reads several.
    warnings: Vec<String>,
    /// The namespaces of the context's rows, for the namespace picker.
    namespaces: Vec<NamespaceSummary>,
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
    /// `load_scope`'s read: each context's built-ins, then its custom kinds,
    /// each applied as it answers. Each part tracks its own progress.
    Load { _task: InFlight },
    /// The live menu's Refresh (clear cache): built-ins and custom kinds in one read.
    Refresh { _task: InFlight },
}

impl Default for ScopeLoad {
    fn default() -> Self {
        Self {
            rows: Load::Loading,
            custom: CustomLoad::Idle,
            custom_interrupted: false,
            custom_ok: false,
            preparing: false,
            read: None,
            contexts: Vec::new(),
            parts: Vec::new(),
        }
    }
}

impl ScopeLoad {
    /// Starts `load_scope`'s read of `contexts`, replacing and so cancelling
    /// any scope read. Every part waits for its context; the contexts in
    /// `custom` also read custom kinds. With `kept`, the table keeps each
    /// context's previous rows until its read answers.
    pub(super) fn start(
        &mut self,
        contexts: Vec<String>,
        custom: &[String],
        kept: bool,
        task: Task<()>,
    ) {
        self.custom = CustomLoad::Idle;
        self.custom_interrupted = false;
        self.custom_ok = false;
        self.preparing = false;
        let mut previous = std::mem::take(&mut self.parts);
        let parts = contexts
            .iter()
            .map(|context| {
                // A kept context keeps its namespaces with its rows.
                let kept = self
                    .position(context)
                    .filter(|&index| kept && previous[index].shown)
                    .map(|index| std::mem::take(&mut previous[index].namespaces));
                ContextRows {
                    shown: kept.is_some(),
                    pending: true,
                    custom_pending: custom.contains(context),
                    namespaces: kept.unwrap_or_default(),
                    ..ContextRows::default()
                }
            })
            .collect();
        self.parts = parts;
        self.contexts = contexts;
        self.read = Some(ScopeRead::Load { _task: task.into() });
    }

    /// Starts the live menu's refresh, replacing and so cancelling any scope read.
    pub(super) fn start_refresh(&mut self, task: Task<()>) {
        self.custom_interrupted |= self.custom() == CustomLoad::Loading;
        self.stop_parts();
        self.read = Some(ScopeRead::Refresh { _task: task.into() });
    }

    /// Drops the scope read, which cancels it.
    pub(super) fn cancel(&mut self) {
        self.stop_parts();
        self.read = None;
    }

    /// Marks every part's read as no longer running, after its read was dropped.
    fn stop_parts(&mut self) {
        self.preparing = false;
        for part in &mut self.parts {
            part.pending = false;
            part.custom_pending = false;
        }
    }

    pub(super) fn set_preparing(&mut self, preparing: bool) {
        self.preparing = preparing;
    }

    /// Applies one context's built-in read, whose rows the caller took. A
    /// failed context shows no rows, one warning and no custom kinds.
    pub(super) fn context_settled(
        &mut self,
        context: &str,
        result: Result<ScopeResult, &AppError>,
    ) {
        let several = self.contexts.len() > 1;
        let Some(index) = self.position(context) else {
            return;
        };
        let part = &mut self.parts[index];
        match result {
            Ok(result) => {
                *part = ContextRows {
                    ready: true,
                    shown: true,
                    pending: false,
                    custom_pending: part.custom_pending,
                    versions: context_versions(context, result.versions),
                    warnings: result
                        .warnings
                        .iter()
                        .map(|warning| context_message(several, context, warning))
                        .collect(),
                    namespaces: result.namespaces,
                };
                self.rows = Load::Ready;
            }
            Err(error) => {
                // A failed context reads no custom kinds.
                let custom = part.custom_pending;
                *part = ContextRows {
                    warnings: vec![format!("Context \"{context}\": {}", error.message)],
                    ..ContextRows::default()
                };
                if custom {
                    self.finish_custom();
                }
            }
        }
    }

    /// The table shows rows of `context`, as when a watch installed them
    /// after its read failed, so its live status counts.
    pub(super) fn mark_shown(&mut self, context: &str) {
        if let Some(index) = self.position(context) {
            self.parts[index].shown = true;
        }
    }

    /// The namespaces of every context whose rows show, once each by name,
    /// in context rank order. `None` while no context's rows show.
    pub(super) fn shown_namespaces(&self) -> Option<Vec<NamespaceSummary>> {
        if !self.parts.iter().any(|part| part.shown) {
            return None;
        }
        let mut namespaces: Vec<NamespaceSummary> = Vec::new();
        for part in self.parts.iter().filter(|part| part.shown) {
            for namespace in &part.namespaces {
                if !namespaces.iter().any(|other| other.name == namespace.name) {
                    namespaces.push(namespace.clone());
                }
            }
        }
        Some(namespaces)
    }

    /// Once no context's custom read runs, the custom kinds fail only when no context read them.
    fn finish_custom(&mut self) {
        if !self.parts.iter().any(|part| part.custom_pending) {
            self.custom = if self.custom_ok {
                CustomLoad::Ready
            } else {
                CustomLoad::Failed
            };
        }
    }

    /// Fails the load when every context failed.
    pub(super) fn all_failed(&mut self, error: String) {
        self.rows = Load::Failed(error);
        self.custom = CustomLoad::Idle;
    }

    /// Applies one context's custom read to the context, whose built-ins
    /// loaded. Returns the rows to add; the warnings join the context's part.
    pub(super) fn custom_settled(
        &mut self,
        context: &str,
        result: Result<ScopeResult, AppError>,
    ) -> Vec<ResourceSummary> {
        let several = self.contexts.len() > 1;
        let Some(part) = self
            .position(context)
            .map(|index| &mut self.parts[index])
            .filter(|part| part.ready)
        else {
            return Vec::new();
        };
        part.custom_pending = false;
        let rows = match result {
            Ok(result) => {
                self.custom_ok = true;
                part.versions
                    .extend(context_versions(context, result.versions));
                part.warnings.extend(
                    result
                        .warnings
                        .iter()
                        .map(|warning| context_message(several, context, warning)),
                );
                result.resources
            }
            Err(error) => {
                part.warnings.push(context_message(
                    several,
                    context,
                    &format!("Custom Resources: {}", error.message),
                ));
                Vec::new()
            }
        };
        self.finish_custom();
        rows
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
                        shown: true,
                        versions: context_versions(&context, result.versions),
                        warnings: result
                            .warnings
                            .iter()
                            .map(|warning| context_message(several, &context, warning))
                            .collect(),
                        namespaces: result.namespaces.clone(),
                        ..ContextRows::default()
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
                    *part = ContextRows {
                        warnings: vec![format!("Context \"{context}\": {}", error.message)],
                        ..ContextRows::default()
                    };
                    applied.warnings.extend(part.warnings.iter().cloned());
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
        let interrupted = std::mem::take(&mut self.custom_interrupted);
        match result {
            Ok(()) => {
                self.rows = Load::Ready;
                self.custom = CustomLoad::Ready;
            }
            // The kept rows lack the custom kinds the refresh cancelled; the
            // summary reports them unavailable until a refresh succeeds.
            Err(_) if matches!(self.rows, Load::Ready) => {
                if interrupted {
                    self.custom = CustomLoad::Failed;
                }
            }
            Err(error) => self.rows = Load::Failed(error),
        }
    }

    /// A context's built-in read or a refresh runs, so the rows may still change.
    pub(super) fn loading(&self) -> bool {
        self.refreshing() || self.parts.iter().any(|part| part.pending)
    }

    /// `context`'s built-in read or a refresh runs, so its rows may still change.
    pub(super) fn context_loading(&self, context: &str) -> bool {
        self.refreshing()
            || self
                .position(context)
                .is_none_or(|index| self.parts[index].pending)
    }

    /// `context`'s custom-kind read runs or waits for its built-ins.
    pub(super) fn custom_loading(&self, context: &str) -> bool {
        self.position(context)
            .is_some_and(|index| self.parts[index].custom_pending)
    }

    /// Watch patches wait: a refresh runs, or rows are being prepared off the UI thread.
    pub(super) fn holds_patches(&self) -> bool {
        self.refreshing() || self.preparing
    }

    /// No context's built-in or custom read runs, so the load is final.
    pub(super) fn settled(&self) -> bool {
        !self
            .parts
            .iter()
            .any(|part| part.pending || part.custom_pending)
    }

    /// The table shows some context's rows: its read in this load succeeded,
    /// its rows were kept, or its watch installed them after its read failed.
    pub(super) fn any_shown(&self) -> bool {
        self.parts.iter().any(|part| part.shown)
    }

    /// The table shows rows of `context`, settled or kept from the previous load.
    pub(super) fn shows(&self, context: &str) -> bool {
        self.position(context)
            .is_some_and(|index| self.parts[index].shown)
    }

    /// A context whose rows the table shows still reads them.
    pub(super) fn shown_loading(&self) -> bool {
        self.refreshing() || self.parts.iter().any(|part| part.shown && part.pending)
    }

    /// Contexts without rows whose read runs while other contexts' rows
    /// show, in rank order, as `loadingContexts` in `contextResults.ts`.
    pub(super) fn waiting_contexts(&self) -> Vec<&str> {
        if !self.parts.iter().any(|part| part.shown) {
            return Vec::new();
        }
        self.contexts
            .iter()
            .zip(&self.parts)
            .filter(|(_, part)| part.pending && !part.shown)
            .map(|(context, _)| context.as_str())
            .collect()
    }

    /// The page's warnings: each context's, in rank order.
    pub(super) fn warnings(&self) -> Vec<String> {
        self.parts
            .iter()
            .flat_map(|part| part.warnings.iter().cloned())
            .collect()
    }

    pub(super) fn refreshing(&self) -> bool {
        matches!(self.read, Some(ScopeRead::Refresh { .. }))
    }

    pub(super) fn custom(&self) -> CustomLoad {
        if self.parts.iter().any(|part| part.custom_pending) {
            CustomLoad::Loading
        } else {
            self.custom
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

    /// The live status of the contexts whose rows the table shows, which the
    /// stale-data notice reads. A context that failed or still loads has no
    /// rows to call stale, as `shownLive` in `ResourceBrowser.svelte`.
    pub(super) fn shown_live_status(&self) -> LiveStatus {
        if self.scope_load.refreshing() {
            LiveStatus::Refreshing
        } else {
            self.live
                .settled_status_where(|context| self.scope_load.shows(context))
        }
    }

    /// Whether row activity may animate: every watch of a context whose rows
    /// show is live and none of those contexts still reads its rows.
    /// Mirrors `updatesCurrent` in `ResourceBrowser.svelte`.
    pub(super) fn updates_current(&self) -> bool {
        !self.scope_load.shown_loading() && self.shown_live_status() == LiveStatus::Live
    }
}
