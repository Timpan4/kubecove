//! Resource observation owns shown rows, list versions, and staged watch changes.
use super::{Load, custom_resources::CustomLoad, request::InFlight};
use crate::backend::{ListVersions, ScopeResult, context_versions};
use crate::model::PreparedRows;
use std::{collections::HashMap, rc::Rc};

mod snapshots;
mod watches;
use crate::workspaces::reads::ContextResults;
use gpui_kit::Task;
use kubecove_lib::models::{AppError, NamespaceSummary, ResourceSummary};
pub(super) use snapshots::{
    ScopeRows, all_failed, context_message, rank_order, same_contexts, union_namespaces,
};
pub(super) use watches::Patch;

pub(super) struct ResourceObservation {
    /// What the table shows for the workspace contexts.
    status: Load,
    data: Rc<PreparedRows>,
    namespace: Option<String>,
    starts: HashMap<String, Option<String>>,
    start_tasks: Vec<Task<()>>,
    pending: Vec<Patch>,
    failures: Vec<(String, AppError)>,
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

/// What applying every context's read changes outside `ResourceObservation`.
#[derive(Default)]
pub(super) struct Applied {
    /// The namespaces of every context that answered, once each by name.
    pub(super) namespaces: Vec<NamespaceSummary>,
    /// The page's warnings, in context rank order.
    pub(super) warnings: Vec<String>,
    /// Contexts whose refresh failed, so the table keeps their last rows.
    pub(super) kept: Vec<String>,
}

/// The scope read in flight. Dropping it cancels the read.
enum ScopeRead {
    /// `load_scope`'s read: each context's built-ins, then its custom kinds,
    /// each applied as it answers. Each part tracks its own progress.
    Load { _task: InFlight },
    /// The live menu's Refresh (clear cache): built-ins and custom kinds in one read.
    Refresh { _task: InFlight },
}

impl Default for ResourceObservation {
    fn default() -> Self {
        Self {
            status: Load::Loading,
            data: Rc::default(),
            namespace: None,
            starts: HashMap::new(),
            start_tasks: Vec::new(),
            pending: Vec::new(),
            failures: Vec::new(),
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

impl ResourceObservation {
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
        self.failures.clear();
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

    /// Applies one context's built-in read, whose rows the caller took. A
    /// failed context shows no rows, one warning and no custom kinds.
    fn context_settled(&mut self, context: &str, result: Result<ScopeResult, &AppError>) {
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
                self.status = Load::Ready;
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
    fn mark_shown(&mut self, context: &str) {
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
    fn all_failed(&mut self, error: String) {
        self.status = Load::Failed(error);
        self.custom = CustomLoad::Idle;
    }

    /// Applies one context's custom read to the context, whose built-ins
    /// loaded. Returns the rows to add; the warnings join the context's part.
    fn custom_settled(
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
    /// context whose rows show keeps its part and rows when it fails, kept
    /// rows whose reload the refresh cancelled included.
    fn apply_parts(&mut self, results: ContextResults<ScopeResult>, refresh: bool) -> Applied {
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
                Err(error) if refresh && part.shown => {
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

    fn has_context(&self, context: &str) -> bool {
        self.position(context).is_some()
    }

    /// The list version of the rows a watch shows, by `watch_id`.
    pub(super) fn version(&self, id: &str) -> Option<&String> {
        self.parts.iter().find_map(|part| part.versions.get(id))
    }

    /// Applies the refresh. A failure keeps rows the table shows; without
    /// rows, as when Refresh is pressed during a first load, it fails the load.
    fn refresh_settled(&mut self, result: Result<(), String>) {
        self.read = None;
        let interrupted = std::mem::take(&mut self.custom_interrupted);
        match result {
            Ok(()) => {
                self.status = Load::Ready;
                self.custom = CustomLoad::Ready;
            }
            // The kept rows lack the custom kinds the refresh cancelled; the
            // summary reports them unavailable until a refresh succeeds.
            Err(_) if matches!(self.status, Load::Ready) => {
                if interrupted {
                    self.custom = CustomLoad::Failed;
                }
            }
            Err(error) => self.status = Load::Failed(error),
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
    fn holds_patches(&self) -> bool {
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
