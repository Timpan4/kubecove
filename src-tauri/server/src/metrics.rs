//! Prometheus metrics (ADR 0027). `observe` updates the event counters as
//! events happen; a scrape adds open sessions, cache state and process usage.
//! Label values are fixed reason codes, declared command names, cache labels
//! and error kinds, never identities or Kubernetes names.

use crate::sessions::Sessions;
use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use kubecove_lib::{commands::ClusterLiveStore, models::BackendCacheDiagnosticSnapshot};
use prometheus_client::{
    encoding::{text::encode, EncodeLabelSet},
    metrics::{counter::Counter, family::Family, gauge::Gauge, histogram::Histogram},
    registry::Registry,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{atomic::AtomicU64, Arc, LazyLock, Mutex},
    time::Duration,
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// Prometheus `client_golang` `DefBuckets`, in seconds (ADR 0027).
const COMMAND_DURATION_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

const CACHE_COUNTERS: [(&str, &str); 5] = [
    ("hits", "Cache reads served from a ready entry"),
    ("misses", "Cache reads that started a load"),
    ("joins", "Cache reads that joined a load in progress"),
    ("evictions", "Cache entries removed for capacity or refresh"),
    (
        "restored_reload_failures",
        "Failed reloads that restored the previous value",
    ),
];

const CACHE_GAUGES: [(&str, &str); 5] = [
    ("ready_entries", "Ready cache entries in open sessions"),
    ("dirty_entries", "Dirty cache entries in open sessions"),
    ("loading_entries", "Loading cache entries in open sessions"),
    (
        "retained_items",
        "Items retained by caches in open sessions",
    ),
    (
        "shallow_payload_bytes",
        "Shallow lower bound of payload bytes retained by caches in open sessions",
    ),
];

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct AuthModeLabel {
    pub auth_mode: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct ReasonLabel {
    pub reason: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct ErrorKindLabel {
    pub error_kind: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct OutcomeLabel {
    pub outcome: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct SignInLabels {
    pub auth_mode: &'static str,
    pub outcome: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct RefusalLabels {
    pub auth_mode: &'static str,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct CommandLabels {
    pub command: &'static str,
    pub outcome: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct CommandErrorLabels {
    pub command: &'static str,
    pub error_kind: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct CacheLabel {
    cache: String,
}

type CommandHistograms = Family<CommandLabels, Histogram, fn() -> Histogram>;

pub struct Metrics {
    registry: Registry,
    pub(crate) sessions_opened: Family<AuthModeLabel, Counter>,
    pub(crate) sessions_resumed: Family<AuthModeLabel, Counter>,
    pub(crate) sessions_closed: Family<ReasonLabel, Counter>,
    pub(crate) sessions_expired: Counter,
    pub(crate) session_cleanup_failures: Family<ErrorKindLabel, Counter>,
    pub(crate) sign_ins: Family<SignInLabels, Counter>,
    pub(crate) authentication_refused: Family<RefusalLabels, Counter>,
    pub(crate) requests_refused: Family<ReasonLabel, Counter>,
    pub(crate) oidc_refreshes: Family<OutcomeLabel, Counter>,
    pub(crate) command_duration: CommandHistograms,
    pub(crate) command_errors: Family<CommandErrorLabels, Counter>,
    sessions_open: Gauge,
    cache_counters: [Family<CacheLabel, Counter>; 5],
    cache_gauges: [Family<CacheLabel, Gauge>; 5],
    resident_memory: Gauge,
    cpu_seconds: Counter<f64, AtomicU64>,
    /// Serializes scrapes, which raise counters by the difference from
    /// their last exported value.
    scrape: tokio::sync::Mutex<Scrape>,
}

#[derive(Default)]
struct Scrape {
    process: System,
    /// Caches that have exported gauges. One missing from a scrape, such as
    /// while its only session is closing, reports zero instead of keeping
    /// its last size.
    caches: BTreeSet<String>,
}

impl Default for Metrics {
    fn default() -> Self {
        let mut metrics = Self {
            registry: Registry::default(),
            sessions_opened: Family::default(),
            sessions_resumed: Family::default(),
            sessions_closed: Family::default(),
            sessions_expired: Counter::default(),
            session_cleanup_failures: Family::default(),
            sign_ins: Family::default(),
            authentication_refused: Family::default(),
            requests_refused: Family::default(),
            oidc_refreshes: Family::default(),
            command_duration: Family::new_with_constructor(|| {
                Histogram::new(COMMAND_DURATION_BUCKETS)
            }),
            command_errors: Family::default(),
            sessions_open: Gauge::default(),
            cache_counters: Default::default(),
            cache_gauges: Default::default(),
            resident_memory: Gauge::default(),
            cpu_seconds: Counter::default(),
            scrape: tokio::sync::Mutex::default(),
        };
        metrics.register();
        metrics
    }
}

impl Metrics {
    fn register(&mut self) {
        let registry = &mut self.registry;
        let prefixed = |name: &str| format!("kubecove_server_{name}");
        registry.register(
            prefixed("sessions_opened"),
            "Sessions opened",
            self.sessions_opened.clone(),
        );
        registry.register(
            prefixed("sessions_resumed"),
            "Sessions resumed by a reconnecting client",
            self.sessions_resumed.clone(),
        );
        registry.register(
            prefixed("sessions_closed"),
            "Sessions closed by the client, logout, shutdown or failed activation",
            self.sessions_closed.clone(),
        );
        registry.register(
            prefixed("sessions_expired"),
            "Sessions that expired after their reconnect grace",
            self.sessions_expired.clone(),
        );
        registry.register(
            prefixed("session_cleanup_failures"),
            "Session cleanups that failed",
            self.session_cleanup_failures.clone(),
        );
        registry.register(
            prefixed("sessions_open"),
            "Sessions currently open",
            self.sessions_open.clone(),
        );
        registry.register(
            prefixed("sign_ins"),
            "Token logins and OIDC callbacks by outcome",
            self.sign_ins.clone(),
        );
        registry.register(
            prefixed("authentication_refused"),
            "Protected requests refused for a missing, invalid or unverifiable credential",
            self.authentication_refused.clone(),
        );
        registry.register(
            prefixed("requests_refused"),
            "Requests refused by the Host or Origin check",
            self.requests_refused.clone(),
        );
        registry.register(
            prefixed("oidc_refreshes"),
            "OIDC token renewals by outcome",
            self.oidc_refreshes.clone(),
        );
        registry.register(
            prefixed("command_duration_seconds"),
            "Command duration by declared command and outcome",
            self.command_duration.clone(),
        );
        registry.register(
            prefixed("command_errors"),
            "Failed commands by declared command and error kind",
            self.command_errors.clone(),
        );
        for ((name, help), family) in CACHE_COUNTERS.iter().zip(&self.cache_counters) {
            registry.register(prefixed(&format!("cache_{name}")), *help, family.clone());
        }
        for ((name, help), family) in CACHE_GAUGES.iter().zip(&self.cache_gauges) {
            registry.register(prefixed(&format!("cache_{name}")), *help, family.clone());
        }
        registry.register(
            "process_resident_memory_bytes",
            "Resident memory size in bytes",
            self.resident_memory.clone(),
        );
        registry.register(
            "process_cpu_seconds",
            "Total user and system CPU time in seconds",
            self.cpu_seconds.clone(),
        );
    }

    pub(crate) fn observe_command(
        &self,
        command: &'static str,
        outcome: &'static str,
        error_kind: Option<&'static str>,
        duration: Duration,
    ) {
        self.command_duration
            .get_or_create(&CommandLabels { command, outcome })
            .observe(duration.as_secs_f64());
        if let Some(error_kind) = error_kind {
            self.command_errors
                .get_or_create(&CommandErrorLabels {
                    command,
                    error_kind,
                })
                .inc();
        }
    }

    /// Records scrape-time values, then encodes the registry.
    pub async fn render(&self, sessions: &Sessions) -> String {
        let mut scrape = self.scrape.lock().await;
        let (open, caches) = sessions.cache_totals().await;
        self.sessions_open
            .set(i64::try_from(open).unwrap_or(i64::MAX));
        for gone in scrape.caches.difference(&caches.keys().cloned().collect()) {
            let label = CacheLabel {
                cache: gone.clone(),
            };
            for family in &self.cache_gauges {
                family.get_or_create(&label).set(0);
            }
        }
        scrape.caches.extend(caches.keys().cloned());
        for (label, totals) in caches {
            let label = CacheLabel { cache: label };
            for (family, total) in self.cache_counters.iter().zip(totals.counters) {
                raise_to(&family.get_or_create(&label), total);
            }
            for (family, value) in self.cache_gauges.iter().zip(totals.gauges) {
                family
                    .get_or_create(&label)
                    .set(i64::try_from(value).unwrap_or(i64::MAX));
            }
        }
        self.sample_process(&mut scrape.process);
        // Encoding under the lock keeps each response one consistent scrape.
        let mut text = String::new();
        encode(&mut text, &self.registry).expect("encoding into a String cannot fail");
        text
    }

    fn sample_process(&self, system: &mut System) {
        let Ok(pid) = sysinfo::get_current_pid() else {
            return;
        };
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_memory()
                .with_cpu(),
        );
        let Some(process) = system.process(pid) else {
            return;
        };
        self.resident_memory
            .set(i64::try_from(process.memory()).unwrap_or(i64::MAX));
        let seconds = Duration::from_millis(process.accumulated_cpu_time()).as_secs_f64();
        let exported = self.cpu_seconds.get();
        if seconds > exported {
            self.cpu_seconds.inc_by(seconds - exported);
        }
    }
}

/// Counters only rise. A session briefly in neither the live list nor the
/// retired totals while it ends lowers the sum for one scrape; holding the
/// exported value keeps that dip from reading as a counter reset.
fn raise_to(counter: &Counter, total: u64) {
    let exported = counter.get();
    if total > exported {
        counter.inc_by(total - exported);
    }
}

/// The process-wide metrics that `observe` updates.
pub fn global() -> &'static Metrics {
    static METRICS: LazyLock<Metrics> = LazyLock::new(Metrics::default);
    &METRICS
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheTotals {
    /// Lifetime counters, in [`CACHE_COUNTERS`] order.
    pub counters: [u64; 5],
    /// Open-session values, in [`CACHE_GAUGES`] order.
    pub gauges: [u64; 5],
}

impl CacheTotals {
    fn add(&mut self, snapshot: &BackendCacheDiagnosticSnapshot, with_gauges: bool) {
        let counters = [
            snapshot.hits,
            snapshot.misses,
            snapshot.joins,
            snapshot.evictions,
            snapshot.restored_reload_failures,
        ];
        for (total, value) in self.counters.iter_mut().zip(counters) {
            *total = total.saturating_add(value);
        }
        if with_gauges {
            let gauges = [
                snapshot.ready,
                snapshot.dirty,
                snapshot.loading,
                snapshot.retained_items,
                snapshot.shallow_payload_bytes,
            ];
            for (total, value) in self.gauges.iter_mut().zip(gauges) {
                *total = total.saturating_add(u64::try_from(value).unwrap_or(u64::MAX));
            }
        }
    }

    /// Adds every cache of an open session's store.
    pub fn add_live(totals: &mut BTreeMap<String, Self>, store: &ClusterLiveStore) {
        for snapshot in store.lifetime_diagnostics() {
            totals
                .entry(snapshot.label.clone())
                .or_default()
                .add(&snapshot, true);
        }
    }
}

/// Lifetime cache counters of ended sessions. Their caches are gone, so only
/// the counters remain.
#[derive(Default)]
pub struct RetiredCaches(Mutex<BTreeMap<String, CacheTotals>>);

impl RetiredCaches {
    pub fn retire(&self, store: &ClusterLiveStore) {
        let snapshots = store.lifetime_diagnostics();
        let mut retired = self.0.lock().expect("retired cache totals lock");
        for snapshot in &snapshots {
            retired
                .entry(snapshot.label.clone())
                .or_default()
                .add(snapshot, false);
        }
    }

    pub fn totals(&self) -> BTreeMap<String, CacheTotals> {
        self.0.lock().expect("retired cache totals lock").clone()
    }
}

/// The metrics listener's router. It shares no route, cookie or
/// authentication state with the main API.
pub fn router(sessions: Arc<Sessions>) -> Router {
    Router::new()
        .route("/metrics", get(scrape))
        .with_state(sessions)
}

async fn scrape(State(sessions): State<Arc<Sessions>>) -> Response {
    let text = global().render(&sessions).await;
    (
        [(
            header::CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )],
        text,
    )
        .into_response()
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
