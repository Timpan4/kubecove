//! View-owned streams. Recovery follows resource-watch.ts; Rust owns transport backoff.
use super::{
    Inspector, Page,
    live_status::WatchReport,
    request::InFlight,
    watch_status::{self, Link, LiveStatus, WatchLink, WatchSummary},
};
use crate::backend::{LogStream, Scope};
use futures_util::future::join_all;
use gpui_kit::{Context, Task, Window};
use kubecove_lib::models::{
    AppError, DiscoveredResourceKind, StreamMessage, WatchResourceKey, WatchResourceKind,
};
use std::{
    collections::{HashMap, HashSet},
    future::{Future, poll_fn},
    pin::Pin,
    task::Poll,
    time::Duration,
};

/// resource-watch.ts::RESOURCE_RECOVERY_INTERVAL.
pub(super) const RECOVERY: Duration = Duration::from_millis(15_000);
/// live-view-refresh.ts::createWatchRefreshThrottle.
pub const COALESCE: Duration = Duration::from_millis(250);

mod watch_task;

/// The page and scope a set of watches belongs to.
struct WatchOwner {
    page: Page,
    launcher: Option<u64>,
    context: String,
    /// Sorted, so the same contexts in another order keep their watches.
    contexts: Vec<String>,
    namespace: Option<String>,
}

/// The kind selection a set of watches was reconciled with.
struct KindScope {
    /// The API version and kind of the selected custom kind.
    selected: Option<(String, String)>,
    chosen: Option<HashSet<String>>,
}

#[derive(Default)]
pub(super) struct LiveData {
    /// The page and scope the watches belong to. A change stops every watch.
    owner: Option<WatchOwner>,
    /// Counts owner changes. A watch task stops once the epoch it started in passes.
    owner_epoch: u64,
    /// The kind selection the watches were last reconciled with. A change
    /// stops only the watches whose kind left the selection.
    kind_scope: Option<KindScope>,
    /// Custom kinds present in the selected namespace, for the sidebar.
    kinds: Vec<DiscoveredResourceKind>,
    /// Counts replacements of `kinds`, so readers derived from them rebuild only on a change.
    kinds_revision: u64,
    /// Every discovered kind in the cluster, for the kind picker as the web's `resourceKindsQuery` provides.
    pub catalog: Vec<DiscoveredResourceKind>,
    pub custom_open: bool,
    pub groups: std::collections::HashSet<String>,
    pub picker_open: bool,
    pub selected: Option<DiscoveredResourceKind>,
    pub kind_locked: bool,
    pub refresh_error: Option<String>,
    pub error_open: Option<super::live_status::NoticeSurface>,
    discovery: Option<Task<()>>,
    catalog_task: Option<Task<()>>,
    /// Kind discovery's warnings, which join the scope read's on each apply.
    pub(super) discovery_warnings: Vec<String>,
    /// Each watch by `watch_id`, so the same kind in two contexts has two watches.
    watches: HashMap<String, Registration>,
}

#[derive(Default)]
struct Registration {
    /// The context the watch runs in, which names its messages when the view reads several.
    context: String,
    phase: WatchPhase,
    reload_error: Option<String>,
    catching_up: bool,
    recycled: Vec<kubecove_lib::models::ResourceSummary>,
}

/// A watch's lifecycle, as `WatchPhase` in resource-watch.ts.
#[derive(Default)]
enum WatchPhase {
    /// The rows for the key have not settled, so the watch waits to start at their version.
    #[default]
    Waiting,
    /// The stream task runs; dropping the phase cancels it.
    Running { _task: InFlight, link: Link },
}

impl Registration {
    fn link(&mut self) -> Option<&mut Link> {
        match &mut self.phase {
            WatchPhase::Waiting => None,
            WatchPhase::Running { link, .. } => Some(link),
        }
    }

    fn summary(&self) -> WatchSummary {
        WatchSummary {
            link: match &self.phase {
                WatchPhase::Waiting => WatchLink::Opening,
                WatchPhase::Running { link, .. } => link.summary(),
            },
            refresh_failed: self.reload_error.is_some(),
            catching_up: self.catching_up,
        }
    }

    /// The stream's error and its denial.
    fn link_messages(&self) -> [Option<&String>; 2] {
        match &self.phase {
            WatchPhase::Waiting => [None, None],
            WatchPhase::Running { link, .. } => link.messages(),
        }
    }
}

impl LiveData {
    /// The watch `id`, registered for `context` if it is new.
    fn registration(&mut self, id: String, context: &str) -> &mut Registration {
        self.watches.entry(id).or_insert_with(|| Registration {
            context: context.to_owned(),
            ..Default::default()
        })
    }
    fn take_buffer(&mut self, id: &str) -> Vec<kubecove_lib::models::ResourceSummary> {
        self.watches
            .get_mut(id)
            .map_or_default(|watch| std::mem::take(&mut watch.recycled))
    }
    pub fn reset_scope(&mut self) {
        self.watches.clear();
        self.owner = None;
        self.refresh_error = None;
    }
    pub fn kinds(&self) -> &[DiscoveredResourceKind] {
        &self.kinds
    }
    pub(super) fn kinds_revision(&self) -> u64 {
        self.kinds_revision
    }
    fn set_kinds(&mut self, kinds: Vec<DiscoveredResourceKind>) {
        self.kinds = kinds;
        self.kinds_revision += 1;
    }
    /// The status without a pending refresh, which resource observation owns.
    pub fn settled_status(&self) -> LiveStatus {
        watch_status::aggregate(
            self.refresh_error.is_some(),
            self.watches.values().map(Registration::summary),
        )
    }

    /// The status of the watches whose context `keep` accepts.
    pub fn watch_status_where(&self, keep: impl Fn(&str) -> bool) -> LiveStatus {
        watch_status::aggregate(
            false,
            self.watches
                .values()
                .filter(|watch| keep(&watch.context))
                .map(Registration::summary),
        )
    }

    pub fn reload_error_in(&self, context: &str) -> Option<&str> {
        self.watches
            .iter()
            .filter(|(_, watch)| watch.context == context)
            .filter_map(|(id, watch)| watch.reload_error.as_deref().map(|error| (id, error)))
            .min_by_key(|(id, _)| *id)
            .map(|(_, error)| error)
    }

    /// Each watch for the status menu, in id order, so the first error it
    /// lists does not change between frames.
    pub fn reports(&self) -> Vec<WatchReport<'_>> {
        let mut watches: Vec<_> = self.watches.iter().collect();
        watches.sort_unstable_by_key(|(id, _)| id.as_str());
        watches
            .into_iter()
            .map(|(_, watch)| {
                let [error, denied] = watch.link_messages();
                WatchReport {
                    context: &watch.context,
                    link: watch.summary().link,
                    error: error.map(String::as_str),
                    denied: denied.map(String::as_str),
                    reload_error: watch.reload_error.as_deref(),
                }
            })
            .collect()
    }
}

/// Every context's kinds once each by API identity, in context rank order,
/// and a warning for each context whose read failed.
fn union_kinds(
    contexts: &[String],
    results: Vec<Result<Vec<DiscoveredResourceKind>, AppError>>,
) -> (Vec<DiscoveredResourceKind>, Vec<String>) {
    let several = contexts.len() > 1;
    let mut seen = HashSet::new();
    let mut kinds = Vec::new();
    let mut warnings = Vec::new();
    for (context, result) in contexts.iter().zip(results) {
        match result {
            Ok(found) => kinds.extend(
                found
                    .into_iter()
                    .filter(|kind| seen.insert(super::kind_selection::identity(kind))),
            ),
            Err(error) => warnings.push(super::resource_observation::context_message(
                several,
                context,
                &format!("Custom Resources: {}", error.message),
            )),
        }
    }
    (kinds, warnings)
}

/// Whether `contexts` holds the contexts of `sorted`, in any order.
fn same_contexts(sorted: &[String], contexts: &[String]) -> bool {
    sorted.len() == contexts.len()
        && contexts.iter().all(|context| {
            let count = |list: &[String]| list.iter().filter(|other| *other == context).count();
            count(sorted) == count(contexts)
        })
}

pub(super) fn key(kind: &str, namespace: Option<String>) -> WatchResourceKey {
    let namespaced = !crate::backend::CLUSTER_KINDS.contains(&kind);
    WatchResourceKey {
        resource_kind: WatchResourceKind {
            kind: kind.into(),
            group: None,
            version: None,
            api_version: None,
            plural: None,
            namespaced: Some(namespaced),
        },
        namespace: if namespaced { namespace } else { None },
    }
}

pub(super) enum WatchInput {
    Message(StreamMessage),
    Tick,
    Closed,
}

pub(super) async fn receive_or_tick(
    stream: &mut LogStream,
    timer: &mut Pin<Box<impl Future<Output = ()>>>,
) -> WatchInput {
    let mut receive = Box::pin(stream.messages.recv());
    poll_fn(|cx| {
        if let Poll::Ready(message) = receive.as_mut().poll(cx) {
            return Poll::Ready(message.map_or(WatchInput::Closed, WatchInput::Message));
        }
        if timer.as_mut().poll(cx).is_ready() {
            return Poll::Ready(WatchInput::Tick);
        }
        Poll::Pending
    })
    .await
}

impl Inspector {
    /// Custom kinds present in the selected namespace, or cluster-wide with no
    /// namespace, as the web's `listPresentCustomResourceKinds` returns them,
    /// for each of `contexts`, once each by API identity.
    pub(super) fn discover_kinds(&mut self, contexts: Vec<String>, cx: &mut Context<Self>) {
        self.live.set_kinds(Vec::new());
        self.live.catalog.clear();
        self.row_index.invalidate_filters();
        self.live.discovery_warnings.clear();
        let namespace = self.scope.namespace.clone();
        let backend = self.backend.clone();
        let timings = self.timings.clone();
        // The picker lists every discovered kind whatever the namespace, as the web's kind picker does.
        let catalog_backend = self.backend.clone();
        let catalog_contexts = contexts.clone();
        self.live.catalog_task = Some(cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            let results = join_all(catalog_contexts.iter().map(|context| {
                let (backend, context, executor) =
                    (catalog_backend.clone(), context.clone(), executor.clone());
                crate::read_retry::cancelled_read(
                    move || backend.resource_kinds(context.clone()),
                    move |delay| executor.timer(delay),
                )
            }))
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.observation.contexts() != catalog_contexts.as_slice() {
                    return;
                }
                let (kinds, warnings) = union_kinds(&catalog_contexts, results);
                this.live.catalog = kinds;
                this.row_index.invalidate_filters();
                this.live
                    .discovery_warnings
                    .extend(warnings.iter().cloned());
                this.warnings.extend(warnings);
                cx.notify();
            });
        }));
        self.live.discovery = Some(cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            let results = join_all(contexts.iter().map(|context| {
                let scope = Scope {
                    context: context.clone(),
                    namespace: namespace.clone(),
                };
                let (backend, timings, executor) =
                    (backend.clone(), timings.clone(), executor.clone());
                crate::read_retry::cancelled_read(
                    move || {
                        timings.measure(
                            "list_present_custom_resource_kinds",
                            backend.namespace_custom_kinds(scope.clone()),
                        )
                    },
                    move |delay| executor.timer(delay),
                )
            }))
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.observation.contexts() != contexts.as_slice() {
                    return;
                }
                let (kinds, warnings) = union_kinds(&contexts, results);
                this.live.set_kinds(kinds);
                this.row_index.invalidate_filters();
                if this.fill_custom_selection() {
                    this.filter(cx);
                }
                this.live
                    .discovery_warnings
                    .extend(warnings.iter().cloned());
                this.warnings.extend(warnings);
                cx.notify();
            });
        }));
    }

    /// Reconciles before rendering: changed scopes drop their tasks and stream guards.
    pub(super) fn sync_live(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _profile = crate::frame_profile::scope("live", "sync");
        self.sync_gitops_watches(window, cx);
        // A change of workspace contexts, in any order, clears every watch.
        let owner_changed = !self.live.owner.as_ref().is_some_and(|owner| {
            owner.page == self.page
                && owner.launcher == self.launcher.active
                && owner.context == self.scope.context
                && owner.namespace == self.scope.namespace
                && same_contexts(&owner.contexts, self.watch_contexts())
        });
        if owner_changed {
            self.live.watches.clear();
            self.observation.discard_patches();
            let mut contexts = self.watch_contexts().to_vec();
            contexts.sort();
            self.live.owner = Some(WatchOwner {
                page: self.page,
                launcher: self.launcher.active,
                context: self.scope.context.clone(),
                contexts,
                namespace: self.scope.namespace.clone(),
            });
            self.live.owner_epoch += 1;
        }
        let selected = self
            .live
            .selected
            .as_ref()
            .map(|kind| (kind.api_version.as_str(), kind.kind.as_str()));
        let kinds_changed = !self.live.kind_scope.as_ref().is_some_and(|scope| {
            scope
                .selected
                .as_ref()
                .map(|(api_version, kind)| (api_version.as_str(), kind.as_str()))
                == selected
                && scope.chosen == self.kind_selection.chosen
        });
        if kinds_changed {
            self.live.kind_scope = Some(KindScope {
                selected: selected
                    .map(|(api_version, kind)| (api_version.to_owned(), kind.to_owned())),
                chosen: self.kind_selection.chosen.clone(),
            });
        }
        let Some(keys) = self.live_keys() else {
            return;
        };
        // One watch per context and kind, so each context's rows patch alone.
        let contexts = self.watch_contexts().to_vec();
        let pairs: Vec<_> = contexts
            .iter()
            .flat_map(|context| keys.iter().map(move |key| (context, key)))
            .map(|(context, key)| (context, key, crate::backend::watch_id(context, key)))
            .collect();
        // A changed kind selection keeps the watches of the kinds it still
        // holds, as `update` in resource-watch.ts keeps unchanged keys' streams.
        if owner_changed || kinds_changed {
            let wanted: HashSet<&str> = pairs.iter().map(|(_, _, id)| id.as_str()).collect();
            self.live
                .watches
                .retain(|id, _| wanted.contains(id.as_str()));
            self.kind_selection.retain_watches(&wanted);
            self.observation.retain_watches(&wanted);
        }
        for (context, key, watch) in pairs {
            if self
                .live
                .watches
                .get(&watch)
                .is_some_and(|watch| matches!(watch.phase, WatchPhase::Running { .. }))
            {
                continue;
            }
            // A watch starts at the version of the rows the view shows, so it
            // waits until they settle, unless its start version was read
            // before those rows were listed. Until then it reports as connecting.
            if !self.watch_ready(context, key) {
                self.live.registration(watch, context);
                continue;
            }
            let scope = Scope {
                context: context.clone(),
                namespace: self.scope.namespace.clone(),
            };
            let task = self.resource_watch_task(scope, key.clone(), watch.clone(), window, cx);
            self.live.registration(watch, context).phase = WatchPhase::Running {
                _task: task.into(),
                link: Link::Opening,
            };
        }
    }

    pub(super) fn queue_live_patch(
        &mut self,
        patch: super::resource_observation::Patch,
        cx: &mut Context<Self>,
    ) {
        if patch.superseded(&self.observation) {
            return;
        }
        if patch.watch.is_some() {
            self.invalidate_topology(&patch.scope, cx);
        }
        self.observation.queue(patch);
        cx.notify();
    }

    pub(super) fn flush_live_patches(&mut self, cx: &mut Context<Self>) {
        let _span = crate::frame_profile::scope("watch", "flush");
        let pending = self.observation.ready_patches();
        if pending.is_empty() {
            return;
        }
        self.visible.release_source();
        for patch in pending {
            if patch.superseded(&self.observation) {
                continue;
            }
            self.row_changes.notice(&patch);
            let (rows, change) = self.patch_kind(
                &patch.scope,
                &patch.key,
                patch.rows,
                patch.resource_version,
                patch.names,
            );
            if let Some((source, change)) = change {
                self.row_index.apply_patch(
                    source,
                    &change,
                    self.observation.rows(),
                    &self.cluster.rows,
                );
            }
            if let Some(watch) = patch
                .watch
                .as_deref()
                .and_then(|id| self.live.watches.get_mut(id))
            {
                // The sequential read consumes this buffer on its next producer turn.
                watch.recycled = rows;
            }
        }
        self.update_saved_overview();
        self.filter_search(cx);
        cx.notify();
    }
}
