use super::{
    normalize_cpu_percent, observed_process_count, process_tree_breakdown_from_processes,
    process_tree_cpu_percent, process_tree_memory_bytes, process_tree_pids_from_roots,
    usage_process_child_description_from_webview, UsageProcessView,
};
use crate::usage_webview;
use kubecove_models::AppUsageMetrics;
use std::collections::{HashMap, HashSet};
use sysinfo::Pid;

#[derive(Clone)]
pub struct BenchUsageProcess {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub start_time: u64,
    pub process_name: String,
    pub command: Vec<String>,
    pub raw_cpu_percent: f32,
    pub memory_bytes: u64,
}

impl BenchUsageProcess {
    fn identity_text(&self) -> String {
        usage_webview::process_identity_text_from_parts(
            self.process_name.clone(),
            self.command.iter().cloned(),
        )
    }
}

impl UsageProcessView for BenchUsageProcess {
    fn raw_cpu_percent(&self) -> f32 {
        self.raw_cpu_percent
    }

    fn memory_bytes(&self) -> u64 {
        self.memory_bytes
    }

    fn is_webview_descendant(&self) -> bool {
        usage_webview::is_webview_descendant_process_text(&self.identity_text())
    }

    fn child_label(&self) -> String {
        usage_process_child_label_from_text(&self.identity_text())
    }

    fn child_description(&self) -> String {
        usage_process_child_description_from_webview(self.is_webview_descendant())
    }
}

#[derive(Clone, Copy)]
pub enum BenchUsagePlatform {
    Native,
    MacOs,
    Windows,
}

// The fixture intentionally measures the production collection representation.
#[allow(clippy::implicit_hasher)]
pub fn benchmark_usage_process_attribution(
    processes: HashMap<u32, BenchUsageProcess>,
    root_pid: u32,
    windows_host_names: Vec<String>,
    platform: BenchUsagePlatform,
    cpu_count: usize,
) -> AppUsageMetrics {
    let root_pid = Pid::from_u32(root_pid);
    let root_start_time = processes
        .get(&root_pid.as_u32())
        .map_or(0, |process| process.start_time);
    let mut pids = process_tree_pids_from_roots(
        [root_pid],
        processes
            .iter()
            .map(|(pid, process)| (Pid::from_u32(*pid), process.parent_pid.map(Pid::from_u32))),
    );

    match platform {
        BenchUsagePlatform::Native => {}
        BenchUsagePlatform::MacOs => {
            let candidates = processes
                .iter()
                .filter(|(pid, process)| {
                    !pids.contains(&Pid::from_u32(**pid))
                        && usage_webview::is_macos_orphan_webkit_process_text(
                            &process.identity_text(),
                        )
                })
                .filter_map(|(_, process)| {
                    let identity = process.identity_text();
                    usage_webview::macos_orphan_webview_process_candidate_from_text(
                        process.start_time,
                        &identity,
                    )
                });
            if let Some(start_time) = usage_webview::selected_orphan_webview_cohort_start_time(
                root_start_time,
                candidates,
            ) {
                pids.extend(
                    processes
                        .iter()
                        .filter(|(_, process)| {
                            process.start_time == start_time
                                && usage_webview::is_macos_orphan_webkit_process_text(
                                    &process.identity_text(),
                                )
                        })
                        .map(|(pid, _)| Pid::from_u32(*pid)),
                );
            }
        }
        BenchUsagePlatform::Windows => {
            let webview_roots = processes
                .iter()
                .filter_map(|(pid, process)| {
                    if !usage_webview::is_windows_app_webview_process_text(
                        &windows_host_names,
                        &process.identity_text(),
                    ) {
                        return None;
                    }
                    let is_browser = usage_webview::is_windows_app_webview_browser_process_text(
                        &windows_host_names,
                        &process.identity_text(),
                    );
                    if is_browser
                        || usage_webview::is_orphan_webview_start_candidate(
                            root_start_time,
                            process.start_time,
                        )
                    {
                        Some(Pid::from_u32(*pid))
                    } else {
                        None
                    }
                })
                .collect::<HashSet<_>>();
            if !webview_roots.is_empty() {
                pids.extend(process_tree_pids_from_roots(
                    webview_roots,
                    processes.iter().map(|(pid, process)| {
                        (Pid::from_u32(*pid), process.parent_pid.map(Pid::from_u32))
                    }),
                ));
            }
        }
    }

    let process_count = observed_process_count(&pids, |pid| processes.get(&pid.as_u32()));
    let memory_bytes = process_tree_memory_bytes(
        &pids,
        |pid| processes.get(&pid.as_u32()),
        |process| process.memory_bytes,
    );
    let raw_cpu_percent = process_tree_cpu_percent(
        &pids,
        |pid| processes.get(&pid.as_u32()),
        |process| process.raw_cpu_percent,
    );
    let samples = pids
        .iter()
        .filter_map(|pid| processes.get(&pid.as_u32()).map(|process| (*pid, process)));
    let breakdown = process_tree_breakdown_from_processes(samples, root_pid, cpu_count);
    AppUsageMetrics {
        cpu_percent: normalize_cpu_percent(raw_cpu_percent, cpu_count),
        memory_bytes,
        process_count,
        sampled_at: "2026-10-11T00:00:00Z".to_string(),
        breakdown,
    }
}

fn usage_process_child_label_from_text(process_text: &str) -> String {
    if usage_webview::is_webview_descendant_process_text(process_text) {
        usage_webview::webview_process_role_from_cmd(process_text).unwrap_or_default()
    } else {
        String::new()
    }
}
