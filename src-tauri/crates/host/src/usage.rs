#[cfg(feature = "bench-support")]
pub mod benchmark;
mod memory;
#[cfg(feature = "bench-support")]
pub use benchmark::{BenchUsagePlatform, BenchUsageProcess, benchmark_usage_process_attribution};

#[cfg(target_os = "macos")]
use super::usage_webview::{
    is_macos_orphan_webkit_process, macos_orphan_webview_process_candidate,
    selected_orphan_webview_cohort_start_time,
};
#[cfg(target_os = "windows")]
use super::usage_webview::{is_orphan_webview_start_candidate, is_windows_app_webview_process};
use super::usage_webview::{is_webview_descendant_process, webview_process_role};
#[cfg(target_os = "windows")]
use super::usage_webview::{
    is_windows_app_webview_browser_process, windows_webview_host_exe_names,
};
use chrono::Utc;
use kubecove_models::AppErrorKind;
use kubecove_models::{AppError, AppUsageMemory, AppUsageMetrics, AppUsageMetricsBreakdown};
use memory::{MemoryTotal, WorkingSetBaseline, resident_bytes, sample_process_memory};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use sysinfo::{
    Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, get_current_pid,
};

#[derive(Clone, Default)]
pub struct AppUsageMonitor {
    state: Arc<Mutex<MonitorState>>,
}

#[derive(Default)]
struct MonitorState {
    /// Created on the first sample: `System::new` fails on platforms sysinfo
    /// does not support, and that failure belongs to the sample that needs it.
    system: Option<System>,
    working_set: WorkingSetBaseline,
}

fn usage_error(error: &sysinfo::Error) -> AppError {
    AppError::new(error.to_string(), AppErrorKind::UsageMetrics)
}

impl AppUsageMonitor {
    /// `measure_working_set` resets Linux referenced bits on every sample so
    /// the next sample can report WSS. Other platforms ignore it.
    pub fn sample(&self, measure_working_set: bool) -> Result<AppUsageMetrics, AppError> {
        let current_pid = get_current_pid().map_err(|error| usage_error(&error))?;
        let mut guard = self.state.lock().map_err(|_| {
            AppError::new(
                "usage metrics monitor is unavailable",
                AppErrorKind::UsageMetrics,
            )
        })?;
        let MonitorState {
            system,
            working_set,
        } = &mut *guard;
        let system = match system {
            Some(system) => system,
            empty => empty.insert(System::new().map_err(|error| usage_error(&error))?),
        };

        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().without_tasks(),
        );

        // WebView2 can outlive its parent. Only its named candidates need
        // command lines to recover the host-exe identity before attribution.
        #[cfg(target_os = "windows")]
        {
            let mut identity_pids = process_tree_pids(system, current_pid);
            identity_pids.extend(system.processes().iter().filter_map(|(pid, process)| {
                process
                    .name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains("msedgewebview2")
                    .then_some(*pid)
            }));
            let identity_pids: Vec<Pid> = identity_pids.into_iter().collect();
            system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&identity_pids),
                false,
                ProcessRefreshKind::nothing()
                    .without_tasks()
                    .with_cmd(UpdateKind::Always),
            );
        }

        let pids = usage_process_pids(system, current_pid);
        let pids_to_refresh: Vec<Pid> = pids.iter().copied().collect();
        if !pids_to_refresh.is_empty() {
            system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&pids_to_refresh),
                true,
                ProcessRefreshKind::nothing()
                    .without_tasks()
                    .with_memory()
                    .with_cpu()
                    .with_cmd(UpdateKind::Always),
            );
        }
        let processes: Vec<(Pid, &Process)> = pids
            .iter()
            .filter_map(|pid| system.process(*pid).map(|process| (*pid, process)))
            .collect();
        let memory = sample_process_memory(&processes, working_set, measure_working_set);
        let samples: Vec<(Pid, SampledProcess)> = processes
            .into_iter()
            .zip(memory.memory)
            .map(|((pid, process), memory)| (pid, SampledProcess { process, memory }))
            .collect();

        Ok(usage_metrics_from_samples(
            samples.iter().map(|(pid, process)| (*pid, process)),
            current_pid,
            available_cpu_count(),
            Utc::now().to_rfc3339(),
            memory.working_set_window_ms,
        ))
    }
}

pub async fn get_app_usage_metrics(
    measure_working_set: bool,
    monitor: kubecove_kube::runtime::State<'_, AppUsageMonitor>,
) -> Result<AppUsageMetrics, AppError> {
    let monitor = monitor.inner().clone();
    tokio::task::spawn_blocking(move || monitor.sample(measure_working_set))
        .await
        .map_err(|error| {
            AppError::new(
                format!("usage metrics worker failed: {error}"),
                AppErrorKind::UsageMetrics,
            )
        })?
}

fn available_cpu_count() -> usize {
    std::thread::available_parallelism().map_or(1, usize::from)
}

pub(crate) fn normalize_cpu_percent(raw_cpu_percent: f32, cpu_count: usize) -> f32 {
    if !raw_cpu_percent.is_finite() {
        return 0.0;
    }

    let divisor = cpu_count.max(1) as f32;
    (raw_cpu_percent / divisor).clamp(0.0, 100.0)
}

fn process_tree_pids(system: &System, root_pid: Pid) -> HashSet<Pid> {
    let relations = system
        .processes()
        .iter()
        .map(|(pid, process)| (*pid, process.parent()));
    process_tree_pids_from_roots([root_pid], relations)
}

fn usage_process_pids(system: &System, root_pid: Pid) -> HashSet<Pid> {
    let mut pids = process_tree_pids(system, root_pid);
    add_related_orphan_webview_pids(system, root_pid, &mut pids);
    pids
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn add_related_orphan_webview_pids(_system: &System, _root_pid: Pid, _pids: &mut HashSet<Pid>) {}

#[cfg(target_os = "windows")]
fn add_related_orphan_webview_pids(system: &System, root_pid: Pid, pids: &mut HashSet<Pid>) {
    let Some(root_process) = system.process(root_pid) else {
        return;
    };
    let root_start_time = root_process.start_time();
    let host_exe_names = windows_webview_host_exe_names(root_process);
    if host_exe_names.is_empty() {
        return;
    }

    let webview_roots: HashSet<Pid> = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            if !is_windows_app_webview_process(process, &host_exe_names) {
                return None;
            }
            if is_windows_app_webview_browser_process(process, &host_exe_names)
                || is_orphan_webview_start_candidate(root_start_time, process.start_time())
            {
                Some(*pid)
            } else {
                None
            }
        })
        .collect();
    if webview_roots.is_empty() {
        return;
    }

    let relations = system
        .processes()
        .iter()
        .map(|(pid, process)| (*pid, process.parent()));
    pids.extend(process_tree_pids_from_roots(webview_roots, relations));
}

#[cfg(target_os = "macos")]
fn add_related_orphan_webview_pids(system: &System, root_pid: Pid, pids: &mut HashSet<Pid>) {
    let Some(root_process) = system.process(root_pid) else {
        return;
    };
    let root_start_time = root_process.start_time();
    let candidate_start_time = selected_orphan_webview_cohort_start_time(
        root_start_time,
        system
            .processes()
            .iter()
            .filter(|(pid, process)| !pids.contains(pid) && is_macos_orphan_webkit_process(process))
            .filter_map(|(_, process)| macos_orphan_webview_process_candidate(process)),
    );

    let Some(candidate_start_time) = candidate_start_time else {
        return;
    };

    pids.extend(
        system
            .processes()
            .iter()
            .filter(|(_, process)| {
                is_macos_orphan_webkit_process(process)
                    && process.start_time() == candidate_start_time
            })
            .map(|(pid, _)| *pid),
    );
}

fn process_tree_pids_from_roots(
    root_pids: impl IntoIterator<Item = Pid>,
    relations: impl IntoIterator<Item = (Pid, Option<Pid>)>,
) -> HashSet<Pid> {
    let relations: Vec<(Pid, Option<Pid>)> = relations.into_iter().collect();
    let mut pids: HashSet<Pid> = root_pids.into_iter().collect();
    let mut changed = true;

    while changed {
        changed = false;
        for (pid, parent) in &relations {
            if parent.is_some_and(|parent_pid| pids.contains(&parent_pid)) && pids.insert(*pid) {
                changed = true;
            }
        }
    }

    pids
}

fn usage_metrics_from_samples<'a, T: UsageProcessView + 'a>(
    samples: impl Iterator<Item = (Pid, &'a T)> + Clone,
    root_pid: Pid,
    cpu_count: usize,
    sampled_at: String,
    working_set_window_ms: Option<u64>,
) -> AppUsageMetrics {
    let mut memory = MemoryTotal::default();
    let mut raw_cpu_percent = 0.0;
    let mut process_count = 0;
    for (_, process) in samples.clone() {
        memory.add(process.memory());
        raw_cpu_percent += process.raw_cpu_percent();
        process_count += 1;
    }
    AppUsageMetrics {
        cpu_percent: normalize_cpu_percent(raw_cpu_percent, cpu_count),
        memory: memory.total(),
        working_set_window_ms,
        process_count,
        sampled_at,
        breakdown: process_tree_breakdown_from_processes(samples, root_pid, cpu_count),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsageProcessGroup {
    App,
    WebView,
    OtherChildren,
}

impl UsageProcessGroup {
    fn label(self) -> &'static str {
        match self {
            Self::App => "KubeCove",
            Self::WebView => "WebView",
            Self::OtherChildren => "Other children",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::App => "Rust/Tauri host process",
            Self::WebView => "Embedded WebView browser runtime",
            Self::OtherChildren => "Non-WebView child processes",
        }
    }

    fn child_label(self, index: usize) -> String {
        match self {
            Self::App => "Host process".to_string(),
            Self::WebView => format!("WebView process {index}"),
            Self::OtherChildren => format!("Child process {index}"),
        }
    }
}

#[derive(Default)]
struct UsageProcessGroupTotals {
    raw_cpu_percent: f32,
    memory: MemoryTotal,
    process_count: usize,
    children: Vec<UsageProcessSample>,
}

struct UsageProcessSample {
    label: String,
    description: String,
    raw_cpu_percent: f32,
    memory: AppUsageMemory,
}

impl UsageProcessGroupTotals {
    fn add_sample(&mut self, sample: UsageProcessSample, include_child_sample: bool) {
        self.raw_cpu_percent += sample.raw_cpu_percent;
        self.memory.add(sample.memory);
        self.process_count += 1;
        if include_child_sample {
            self.children.push(sample);
        }
    }
}

trait UsageProcessView {
    fn raw_cpu_percent(&self) -> f32;
    fn memory(&self) -> AppUsageMemory;
    fn is_webview_descendant(&self) -> bool;
    fn child_label(&self) -> String;
    fn child_description(&self) -> String;

    fn group(&self, pid: Pid, root_pid: Pid) -> UsageProcessGroup {
        usage_process_group_with_webview_check(pid, root_pid, || self.is_webview_descendant())
    }
}

/// A sysinfo process with the memory figures sampled for it.
struct SampledProcess<'a> {
    process: &'a Process,
    memory: AppUsageMemory,
}

impl UsageProcessView for SampledProcess<'_> {
    fn raw_cpu_percent(&self) -> f32 {
        self.process.cpu_usage()
    }

    fn memory(&self) -> AppUsageMemory {
        self.memory
    }

    fn is_webview_descendant(&self) -> bool {
        is_webview_descendant_process(self.process)
    }

    fn child_label(&self) -> String {
        usage_process_child_label(self.process)
    }

    fn child_description(&self) -> String {
        usage_process_child_description(self.process)
    }
}

fn process_tree_breakdown_from_processes<'a, T: UsageProcessView + 'a>(
    samples: impl IntoIterator<Item = (Pid, &'a T)>,
    root_pid: Pid,
    cpu_count: usize,
) -> Vec<AppUsageMetricsBreakdown> {
    let prepared_samples = samples.into_iter().map(|(pid, process)| {
        let group = process.group(pid, root_pid);
        let is_child = group != UsageProcessGroup::App;
        let sample = UsageProcessSample {
            label: if is_child {
                process.child_label()
            } else {
                String::new()
            },
            description: if is_child {
                process.child_description()
            } else {
                String::new()
            },
            raw_cpu_percent: process.raw_cpu_percent(),
            memory: process.memory(),
        };
        (group, sample)
    });
    process_tree_breakdown_from_samples(prepared_samples, cpu_count)
}

fn process_tree_breakdown_from_samples(
    samples: impl IntoIterator<Item = (UsageProcessGroup, UsageProcessSample)>,
    cpu_count: usize,
) -> Vec<AppUsageMetricsBreakdown> {
    let mut buckets = [
        (UsageProcessGroup::App, UsageProcessGroupTotals::default()),
        (
            UsageProcessGroup::WebView,
            UsageProcessGroupTotals::default(),
        ),
        (
            UsageProcessGroup::OtherChildren,
            UsageProcessGroupTotals::default(),
        ),
    ];

    for (group, sample) in samples {
        let bucket_index = match group {
            UsageProcessGroup::App => 0,
            UsageProcessGroup::WebView => 1,
            UsageProcessGroup::OtherChildren => 2,
        };
        buckets[bucket_index]
            .1
            .add_sample(sample, bucket_index != 0);
    }

    buckets
        .into_iter()
        .filter(|(_, totals)| totals.process_count > 0)
        .map(|(group, totals)| AppUsageMetricsBreakdown {
            label: group.label().to_string(),
            description: group.description().to_string(),
            cpu_percent: normalize_cpu_percent(totals.raw_cpu_percent, cpu_count),
            memory: totals.memory.total(),
            process_count: totals.process_count,
            children: usage_process_children(group, totals.children, cpu_count),
        })
        .collect()
}

fn usage_process_children(
    group: UsageProcessGroup,
    mut children: Vec<UsageProcessSample>,
    cpu_count: usize,
) -> Vec<AppUsageMetricsBreakdown> {
    children.sort_by_key(|child| std::cmp::Reverse(resident_bytes(&child.memory)));
    let mut label_counts: HashMap<String, usize> = HashMap::new();
    children
        .into_iter()
        .enumerate()
        .map(|(index, child)| AppUsageMetricsBreakdown {
            label: if child.label.is_empty() {
                group.child_label(index + 1)
            } else {
                let count = label_counts.entry(child.label.clone()).or_insert(0);
                *count += 1;
                if *count == 1 {
                    child.label
                } else {
                    format!("{} {}", child.label, count)
                }
            },
            description: child.description,
            cpu_percent: normalize_cpu_percent(child.raw_cpu_percent, cpu_count),
            memory: child.memory,
            process_count: 1,
            children: Vec::new(),
        })
        .collect()
}

fn usage_process_child_label(process: &Process) -> String {
    webview_process_role(process).unwrap_or_default()
}

fn usage_process_child_description(process: &Process) -> String {
    usage_process_child_description_from_webview(is_webview_descendant_process(process))
}

fn usage_process_child_description_from_webview(is_webview: bool) -> String {
    if is_webview {
        "WebView child process".to_string()
    } else {
        "Child process".to_string()
    }
}

fn usage_process_group_with_webview_check(
    pid: Pid,
    root_pid: Pid,
    is_webview: impl FnOnce() -> bool,
) -> UsageProcessGroup {
    if pid == root_pid {
        return UsageProcessGroup::App;
    }

    if is_webview() {
        UsageProcessGroup::WebView
    } else {
        UsageProcessGroup::OtherChildren
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_cpu_percent, process_tree_pids_from_roots};
    use sysinfo::Pid;

    fn pid(value: u32) -> Pid {
        Pid::from_u32(value)
    }

    // Linux thread tasks share their owning process's address space. Including
    // them in the sample counts the same RSS once per thread.
    #[cfg(target_os = "linux")]
    #[test]
    fn samples_processes_without_linux_thread_tasks() {
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let worker = std::thread::spawn(move || {
            let task_path = std::fs::read_link("/proc/thread-self").expect("Linux thread identity");
            let task_pid = task_path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("thread PID")
                .parse::<u32>()
                .expect("numeric thread PID");
            started_tx
                .send(Pid::from_u32(task_pid))
                .expect("started receiver");
            let _ = stop_rx.recv();
        });
        let task_pid = started_rx.recv().expect("worker identity");
        let monitor = super::AppUsageMonitor::default();
        let metrics = monitor.sample(false).expect("usage sample");
        let includes_task = monitor
            .state
            .lock()
            .expect("monitor lock")
            .system
            .as_ref()
            .expect("sampled system")
            .process(task_pid)
            .is_some();
        drop(stop_tx);
        worker.join().expect("worker exit");
        assert!(metrics.process_count >= 1);
        assert!(
            !includes_task,
            "thread tasks must not be sampled as processes"
        );
    }

    // Failure modes: WSS reported before any reset (it would count every page
    // touched since process start), a baseline that survives turning WSS off,
    // and PSS missing because smaps_rollup was not read.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_wss_covers_only_the_interval_since_a_reset() {
        let monitor = super::AppUsageMonitor::default();
        let sample = |measure| monitor.sample(measure).expect("usage sample");

        let first = sample(true);
        assert_eq!(first.memory.wss_bytes, None);
        assert_eq!(first.working_set_window_ms, None);
        assert!(first.memory.rss_bytes.is_some_and(|bytes| bytes > 0));
        assert!(first.memory.pss_bytes.is_some_and(|bytes| bytes > 0));

        let second = sample(true);
        assert!(second.memory.wss_bytes.is_some_and(|bytes| bytes > 0));
        assert!(second.working_set_window_ms.is_some());

        let disabled = sample(false);
        assert_eq!(disabled.memory.wss_bytes, None);
        assert_eq!(disabled.working_set_window_ms, None);
        assert_eq!(sample(true).memory.wss_bytes, None);
    }

    #[test]
    fn normalizes_process_tree_cpu_to_host_percentage() {
        assert!((normalize_cpu_percent(240.0, 8) - 30.0).abs() < f32::EPSILON);
        assert!((normalize_cpu_percent(250.0, 0) - 100.0).abs() < f32::EPSILON);
        assert!(normalize_cpu_percent(f32::NAN, 8).abs() < f32::EPSILON);
    }

    #[test]
    fn collects_recursive_child_processes() {
        let pids = process_tree_pids_from_roots(
            [pid(10)],
            [
                (pid(10), None),
                (pid(11), Some(pid(10))),
                (pid(12), Some(pid(11))),
                (pid(20), None),
            ],
        );

        assert!(pids.contains(&pid(10)));
        assert!(pids.contains(&pid(11)));
        assert!(pids.contains(&pid(12)));
        assert!(!pids.contains(&pid(20)));
    }

    #[test]
    fn ignores_processes_without_parent_link_to_host() {
        let pids = process_tree_pids_from_roots(
            [pid(10)],
            [
                (pid(10), None),
                (pid(11), Some(pid(10))),
                (pid(30), None),
                (pid(31), Some(pid(30))),
            ],
        );

        assert_eq!(pids, [pid(10), pid(11)].into_iter().collect());
    }

    #[test]
    fn collects_multiple_process_roots_for_reused_webview_browser() {
        let pids = process_tree_pids_from_roots(
            [pid(10), pid(30)],
            [
                (pid(10), None),
                (pid(30), Some(pid(99))),
                (pid(31), Some(pid(30))),
                (pid(32), Some(pid(30))),
                (pid(40), None),
                (pid(41), Some(pid(40))),
            ],
        );

        assert_eq!(
            pids,
            [pid(10), pid(30), pid(31), pid(32)].into_iter().collect()
        );
    }
}
