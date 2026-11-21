#[cfg(target_os = "macos")]
use super::super::usage_webview::macos_orphan_webview_process_candidate;
use super::super::usage_webview::{
    OrphanWebViewProcessCandidate, is_orphan_webview_start_candidate,
    is_windows_app_webview_browser_process_text, is_windows_app_webview_process_text,
    selected_orphan_webview_cohort_start_time,
};
#[cfg(target_os = "windows")]
use super::super::usage_webview::{process_identity_text, windows_webview_host_exe_names};
use std::collections::{HashMap, HashSet};
use sysinfo::{Pid, System};

pub(super) fn process_tree_pids(
    system: &System,
    root_pids: impl IntoIterator<Item = Pid>,
) -> HashSet<Pid> {
    process_tree_pids_from_roots(root_pids, process_relations(system))
}

fn process_relations(system: &System) -> impl Iterator<Item = (Pid, Option<Pid>)> {
    system
        .processes()
        .iter()
        .map(|(pid, process)| (*pid, process.parent()))
}

pub(super) fn usage_process_pids(system: &System, root_pid: Pid) -> HashSet<Pid> {
    #[cfg(target_os = "windows")]
    if let Some(root) = system.process(root_pid) {
        return windows_usage_process_pids(
            root_pid,
            process_relations(system),
            root.start_time(),
            &windows_webview_host_exe_names(root),
            system
                .processes()
                .iter()
                .map(|(&pid, process)| (pid, process.start_time(), process_identity_text(process))),
        );
    }
    #[cfg(target_os = "macos")]
    if let Some(root) = system.process(root_pid) {
        return macos_usage_process_pids(
            root_pid,
            process_relations(system),
            root.start_time(),
            system.processes().iter().filter_map(|(&pid, process)| {
                macos_orphan_webview_process_candidate(process).map(|candidate| (pid, candidate))
            }),
        );
    }
    process_tree_pids(system, [root_pid])
}

// Windows root selection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
fn windows_usage_process_pids(
    root_pid: Pid,
    relations: impl IntoIterator<Item = (Pid, Option<Pid>)>,
    root_start_time: u64,
    host_exe_names: &[String],
    processes: impl IntoIterator<Item = (Pid, u64, impl AsRef<str>)>,
) -> HashSet<Pid> {
    if host_exe_names.is_empty() {
        return process_tree_pids_from_roots([root_pid], relations);
    }
    let webview_roots = processes
        .into_iter()
        .filter_map(|(pid, start_time, identity)| {
            let identity = identity.as_ref();
            (is_windows_app_webview_process_text(host_exe_names, identity)
                && (is_orphan_webview_start_candidate(root_start_time, start_time)
                    || is_windows_app_webview_browser_process_text(host_exe_names, identity)))
            .then_some(pid)
        });
    process_tree_pids_from_roots(std::iter::once(root_pid).chain(webview_roots), relations)
}

// macOS root selection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
fn macos_usage_process_pids(
    root_pid: Pid,
    relations: impl IntoIterator<Item = (Pid, Option<Pid>)>,
    root_start_time: u64,
    candidates: impl Iterator<Item = (Pid, OrphanWebViewProcessCandidate)> + Clone,
) -> HashSet<Pid> {
    let mut pids = process_tree_pids_from_roots([root_pid], relations);
    let candidate_start_time = selected_orphan_webview_cohort_start_time(
        root_start_time,
        candidates
            .clone()
            .filter_map(|(pid, candidate)| (!pids.contains(&pid)).then_some(candidate)),
    );

    let Some(candidate_start_time) = candidate_start_time else {
        return pids;
    };

    pids.extend(candidates.filter_map(|(pid, candidate)| {
        (candidate.start_time == candidate_start_time).then_some(pid)
    }));
    pids
}

pub(super) fn process_tree_pids_from_roots(
    root_pids: impl IntoIterator<Item = Pid>,
    relations: impl IntoIterator<Item = (Pid, Option<Pid>)>,
) -> HashSet<Pid> {
    let mut children_by_parent: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (pid, parent) in relations {
        if let Some(parent) = parent {
            children_by_parent.entry(parent).or_default().push(pid);
        }
    }

    let mut pids: HashSet<Pid> = root_pids.into_iter().collect();
    let mut pending: Vec<Pid> = pids.iter().copied().collect();
    while let Some(parent) = pending.pop() {
        if let Some(children) = children_by_parent.get(&parent) {
            for &child in children {
                if pids.insert(child) {
                    pending.push(child);
                }
            }
        }
    }

    pids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage_webview::macos_orphan_webview_process_candidate_from_text;
    use sysinfo::Pid;

    fn pid(value: u32) -> Pid {
        Pid::from_u32(value)
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
    fn windows_usage_includes_detached_root_and_its_descendants() {
        let pids = windows_usage_process_pids(
            pid(10),
            [
                (pid(10), None),
                (pid(11), Some(pid(10))),
                (pid(30), Some(pid(99))),
                (pid(31), Some(pid(30))),
                (pid(32), None),
                (pid(33), None),
                (pid(40), None),
                (pid(41), Some(pid(40))),
            ],
            200,
            &["kubecove.exe".into()],
            [
                (
                    pid(30),
                    100,
                    "msedgewebview2.exe --webview-exe-name=kubecove.exe --embedded-browser-webview=1",
                ),
                (
                    pid(32),
                    201,
                    "msedgewebview2.exe --webview-exe-name=kubecove.exe --type=renderer",
                ),
                (
                    pid(33),
                    100,
                    "msedgewebview2.exe --webview-exe-name=kubecove.exe --type=renderer",
                ),
                (
                    pid(40),
                    201,
                    "msedgewebview2.exe --webview-exe-name=other.exe --embedded-browser-webview=1",
                ),
            ],
        );

        assert_eq!(
            pids,
            [pid(10), pid(11), pid(30), pid(31), pid(32)]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn macos_usage_excludes_attributed_descendants_from_orphan_cohort_selection() {
        let webviews = [
            (pid(11), 201, "com.apple.WebKit.WebContent"),
            (pid(30), 201, "com.apple.WebKit.WebContent"),
            (pid(31), 201, "com.apple.WebKit.Networking"),
            (pid(40), 202, "com.apple.WebKit.WebContent"),
            (pid(50), 201, "unrelated-process"),
        ];
        let pids = macos_usage_process_pids(
            pid(10),
            [
                (pid(10), None),
                (pid(11), Some(pid(10))),
                (pid(12), Some(pid(11))),
                (pid(30), None),
                (pid(31), None),
                (pid(40), None),
                (pid(50), None),
            ],
            200,
            webviews.into_iter().filter_map(|(pid, started, identity)| {
                macos_orphan_webview_process_candidate_from_text(started, identity)
                    .map(|candidate| (pid, candidate))
            }),
        );

        assert_eq!(
            pids,
            [pid(10), pid(11), pid(12), pid(30), pid(31)]
                .into_iter()
                .collect()
        );
    }
}
