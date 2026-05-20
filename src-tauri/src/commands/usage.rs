use crate::models::{AppError, AppUsageMetrics, AppUsageMetricsBreakdown};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use sysinfo::{
    get_current_pid, Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind,
};

#[derive(Default)]
pub struct AppUsageMonitor {
    system: Mutex<System>,
}

impl AppUsageMonitor {
    pub fn sample(&self) -> Result<AppUsageMetrics, AppError> {
        let current_pid =
            get_current_pid().map_err(|message| AppError::new(message, "usage_metrics"))?;
        let mut system = self
            .system
            .lock()
            .map_err(|_| AppError::new("usage metrics monitor is unavailable", "usage_metrics"))?;

        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_memory()
                .with_cpu()
                .with_cmd(UpdateKind::Always),
        );

        let pids = process_tree_pids(&system, current_pid);
        let process_count = observed_process_count(&system, &pids);
        let memory_bytes = process_tree_memory_bytes(&system, &pids);
        let raw_cpu_percent = process_tree_cpu_percent(&system, &pids);
        let cpu_count = available_cpu_count();
        let cpu_percent = normalize_cpu_percent(raw_cpu_percent, cpu_count);
        let breakdown = process_tree_breakdown(&system, &pids, current_pid, cpu_count);

        Ok(AppUsageMetrics {
            cpu_percent,
            memory_bytes,
            process_count,
            sampled_at: Utc::now().to_rfc3339(),
            breakdown,
        })
    }
}

#[tauri::command]
pub fn get_app_usage_metrics(
    monitor: tauri::State<'_, AppUsageMonitor>,
) -> Result<AppUsageMetrics, AppError> {
    monitor.sample()
}

fn available_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
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
    process_tree_pids_from_relations(root_pid, relations)
}

pub(crate) fn process_tree_pids_from_relations(
    root_pid: Pid,
    relations: impl IntoIterator<Item = (Pid, Option<Pid>)>,
) -> HashSet<Pid> {
    let relations: Vec<(Pid, Option<Pid>)> = relations.into_iter().collect();
    let mut pids = HashSet::from([root_pid]);
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

fn observed_process_count(system: &System, pids: &HashSet<Pid>) -> usize {
    pids.iter()
        .filter(|pid| system.process(**pid).is_some())
        .count()
}

fn process_tree_memory_bytes(system: &System, pids: &HashSet<Pid>) -> u64 {
    pids.iter()
        .filter_map(|pid| system.process(*pid))
        .map(|process| process.memory())
        .sum()
}

fn process_tree_cpu_percent(system: &System, pids: &HashSet<Pid>) -> f32 {
    pids.iter()
        .filter_map(|pid| system.process(*pid))
        .map(|process| process.cpu_usage())
        .sum()
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
    memory_bytes: u64,
    process_count: usize,
    children: Vec<UsageProcessSample>,
}

struct UsageProcessSample {
    label: String,
    description: String,
    raw_cpu_percent: f32,
    memory_bytes: u64,
}

impl UsageProcessGroupTotals {
    fn add_process(&mut self, process: &Process, include_child_sample: bool) {
        self.raw_cpu_percent += process.cpu_usage();
        self.memory_bytes += process.memory();
        self.process_count += 1;
        if include_child_sample {
            self.children.push(UsageProcessSample {
                label: usage_process_child_label(process),
                description: usage_process_child_description(process),
                raw_cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
            });
        }
    }
}

fn process_tree_breakdown(
    system: &System,
    pids: &HashSet<Pid>,
    root_pid: Pid,
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

    for pid in pids {
        let Some(process) = system.process(*pid) else {
            continue;
        };
        let group = usage_process_group(*pid, root_pid, &process_name_lossy(process));
        let bucket_index = match group {
            UsageProcessGroup::App => 0,
            UsageProcessGroup::WebView => 1,
            UsageProcessGroup::OtherChildren => 2,
        };
        buckets[bucket_index]
            .1
            .add_process(process, group != UsageProcessGroup::App);
    }

    buckets
        .into_iter()
        .filter(|(_, totals)| totals.process_count > 0)
        .map(|(group, totals)| AppUsageMetricsBreakdown {
            label: group.label().to_string(),
            description: group.description().to_string(),
            cpu_percent: normalize_cpu_percent(totals.raw_cpu_percent, cpu_count),
            memory_bytes: totals.memory_bytes,
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
    children.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
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
            memory_bytes: child.memory_bytes,
            process_count: 1,
            children: Vec::new(),
        })
        .collect()
}

fn usage_process_child_label(process: &Process) -> String {
    webview_process_role(process).unwrap_or_default()
}

fn usage_process_child_description(process: &Process) -> String {
    if is_webview_process_name(&process_name_lossy(process)) {
        "WebView child process".to_string()
    } else {
        "Child process".to_string()
    }
}

fn process_name_lossy(process: &Process) -> String {
    process.name().to_string_lossy().into_owned()
}

fn process_cmd_text(process: &Process) -> String {
    process
        .cmd()
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn webview_process_role_from_cmd(command_line: &str) -> Option<String> {
    let text = command_line.to_ascii_lowercase();

    if text.contains("--type=renderer") {
        return Some("Renderer".to_string());
    }
    if text.contains("--type=gpu-process") {
        return Some("GPU process".to_string());
    }
    if text.contains("--type=utility") && text.contains("network.mojom.networkservice") {
        return Some("Network service".to_string());
    }
    if text.contains("--type=utility") && text.contains("storage.mojom.storageservice") {
        return Some("Storage service".to_string());
    }
    if text.contains("--type=utility") && text.contains("audio.mojom.audioservice") {
        return Some("Audio service".to_string());
    }
    if text.contains("--type=utility") {
        return Some("Utility process".to_string());
    }
    if text.contains("--type=crashpad-handler") || text.contains("crashpad") {
        return Some("Crash handler".to_string());
    }
    if text.contains("msedgewebview2") || text.contains("webview") {
        return Some("Browser process".to_string());
    }

    None
}

fn webview_process_role(process: &Process) -> Option<String> {
    if !is_webview_process_name(&process_name_lossy(process)) {
        return None;
    }
    webview_process_role_from_cmd(&process_cmd_text(process))
        .or_else(|| Some("WebView process".to_string()))
}

fn usage_process_group(pid: Pid, root_pid: Pid, process_name: &str) -> UsageProcessGroup {
    if pid == root_pid {
        return UsageProcessGroup::App;
    }

    if is_webview_process_name(process_name) {
        UsageProcessGroup::WebView
    } else {
        UsageProcessGroup::OtherChildren
    }
}

pub(crate) fn is_webview_process_name(process_name: &str) -> bool {
    let name = process_name.to_ascii_lowercase();
    name.contains("webview") || name.contains("msedgewebview")
}

#[cfg(test)]
mod tests {
    use super::{
        is_webview_process_name, normalize_cpu_percent, process_tree_pids_from_relations,
        webview_process_role_from_cmd,
    };
    use sysinfo::Pid;

    fn pid(value: u32) -> Pid {
        Pid::from_u32(value)
    }

    #[test]
    fn normalizes_process_tree_cpu_to_host_percentage() {
        assert_eq!(normalize_cpu_percent(240.0, 8), 30.0);
        assert_eq!(normalize_cpu_percent(250.0, 0), 100.0);
        assert_eq!(normalize_cpu_percent(f32::NAN, 8), 0.0);
    }

    #[test]
    fn collects_recursive_child_processes() {
        let pids = process_tree_pids_from_relations(
            pid(10),
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
    fn identifies_webview_process_names() {
        assert!(is_webview_process_name("msedgewebview2.exe"));
        assert!(is_webview_process_name("Microsoft.WebView2"));
        assert!(!is_webview_process_name("kubecove.exe"));
    }

    #[test]
    fn classifies_webview_roles_from_command_flags() {
        assert_eq!(
            webview_process_role_from_cmd("msedgewebview2.exe --type=renderer"),
            Some("Renderer".to_string()),
        );
        assert_eq!(
            webview_process_role_from_cmd("msedgewebview2.exe --type=gpu-process"),
            Some("GPU process".to_string()),
        );
        assert_eq!(
            webview_process_role_from_cmd(
                "msedgewebview2.exe --type=utility --utility-sub-type=network.mojom.NetworkService",
            ),
            Some("Network service".to_string()),
        );
    }
}
