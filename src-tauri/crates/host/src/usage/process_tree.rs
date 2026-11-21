#[cfg(target_os = "macos")]
use super::super::usage_webview::{
    is_macos_orphan_webkit_process, macos_orphan_webview_process_candidate,
    selected_orphan_webview_cohort_start_time,
};
#[cfg(target_os = "windows")]
use super::super::usage_webview::{
    is_orphan_webview_start_candidate, is_windows_app_webview_process,
};
#[cfg(target_os = "windows")]
use super::super::usage_webview::{
    is_windows_app_webview_browser_process, windows_webview_host_exe_names,
};
use std::collections::{HashMap, HashSet};
use sysinfo::{Pid, System};

pub(super) fn process_tree_pids(
    system: &System,
    root_pids: impl IntoIterator<Item = Pid>,
) -> HashSet<Pid> {
    let relations = system
        .processes()
        .iter()
        .map(|(pid, process)| (*pid, process.parent()));
    process_tree_pids_from_roots(root_pids, relations)
}

pub(super) fn usage_process_pids(system: &System, root_pid: Pid) -> HashSet<Pid> {
    let roots = [root_pid];
    #[cfg(target_os = "windows")]
    let roots = roots
        .into_iter()
        .chain(windows_webview_roots(system, root_pid));
    let pids = process_tree_pids(system, roots);
    #[cfg(target_os = "macos")]
    let pids = with_macos_orphan_webview_pids(system, root_pid, pids);
    pids
}

#[cfg(target_os = "windows")]
fn windows_webview_roots(system: &System, root_pid: Pid) -> Vec<Pid> {
    let Some(root_process) = system.process(root_pid) else {
        return Vec::new();
    };
    let root_start_time = root_process.start_time();
    let host_exe_names = windows_webview_host_exe_names(root_process);
    if host_exe_names.is_empty() {
        return Vec::new();
    }

    system
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
        .collect()
}

#[cfg(target_os = "macos")]
fn with_macos_orphan_webview_pids(
    system: &System,
    root_pid: Pid,
    mut pids: HashSet<Pid>,
) -> HashSet<Pid> {
    let Some(root_process) = system.process(root_pid) else {
        return pids;
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
        return pids;
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
    use super::process_tree_pids_from_roots;
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
