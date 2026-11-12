//! Per-process WSS, RSS and PSS figures for the usage footer.
//!
//! sysinfo's `Process::memory()` is the working set on Windows and the
//! resident set on Linux and macOS. Linux PSS comes from
//! `/proc/<pid>/smaps_rollup`. Linux WSS is the `Referenced` total after the
//! previous sample wrote `1` to `/proc/<pid>/clear_refs`, so it covers the
//! interval between two samples. macOS has no per-page referenced interface,
//! so it reports no WSS.

use kubecove_models::AppUsageMemory;
use std::collections::HashMap;
use std::time::Instant;
use sysinfo::{Pid, Process};

/// The processes whose referenced bits the previous Linux sample reset.
#[derive(Default)]
// Linux WSS sampling is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) struct WorkingSetBaseline {
    reset_at: Option<Instant>,
    /// Process start time by PID, so a reused PID never inherits a baseline.
    start_times: HashMap<Pid, u64>,
}

pub(super) struct MemorySample {
    /// One entry per input process, in input order.
    pub memory: Vec<AppUsageMemory>,
    pub working_set_window_ms: Option<u64>,
}

/// Sums each figure across processes. A figure that any process lacks is
/// `None` in the total.
#[derive(Default)]
pub(super) struct MemoryTotal(Option<AppUsageMemory>);

impl MemoryTotal {
    pub(super) fn add(&mut self, memory: AppUsageMemory) {
        self.0 = Some(match self.0 {
            None => memory,
            Some(total) => AppUsageMemory {
                wss_bytes: add_figure(total.wss_bytes, memory.wss_bytes),
                rss_bytes: add_figure(total.rss_bytes, memory.rss_bytes),
                pss_bytes: add_figure(total.pss_bytes, memory.pss_bytes),
            },
        });
    }

    pub(super) fn total(self) -> AppUsageMemory {
        self.0.unwrap_or_default()
    }
}

fn add_figure(total: Option<u64>, sample: Option<u64>) -> Option<u64> {
    total?.checked_add(sample?)
}

/// The figure the process breakdown sorts by: resident memory on every platform.
pub(super) fn resident_bytes(memory: &AppUsageMemory) -> u64 {
    memory.rss_bytes.or(memory.wss_bytes).unwrap_or(0)
}

/// Maps sysinfo's `Process::memory()` to the figure it represents on this platform.
fn sysinfo_memory(bytes: u64) -> AppUsageMemory {
    if cfg!(target_os = "windows") {
        AppUsageMemory {
            wss_bytes: Some(bytes),
            ..AppUsageMemory::default()
        }
    } else {
        AppUsageMemory {
            rss_bytes: Some(bytes),
            ..AppUsageMemory::default()
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) fn sample_process_memory(
    processes: &[(Pid, &Process)],
    _baseline: &mut WorkingSetBaseline,
    _measure_working_set: bool,
) -> MemorySample {
    MemorySample {
        memory: processes
            .iter()
            .map(|(_, process)| sysinfo_memory(process.memory()))
            .collect(),
        working_set_window_ms: None,
    }
}

#[cfg(target_os = "linux")]
pub(super) fn sample_process_memory(
    processes: &[(Pid, &Process)],
    baseline: &mut WorkingSetBaseline,
    measure_working_set: bool,
) -> MemorySample {
    let previous = std::mem::take(baseline);
    let mut memory = Vec::with_capacity(processes.len());
    for (pid, process) in processes {
        let rollup = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup"))
            .map_or_default(|text| parse_smaps_rollup(&text));
        let has_baseline = previous.start_times.get(pid) == Some(&process.start_time());
        let wss_bytes = if measure_working_set && has_baseline {
            rollup.referenced_bytes
        } else {
            None
        };
        if measure_working_set && reset_referenced_pages(*pid).is_ok() {
            baseline.start_times.insert(*pid, process.start_time());
        }
        memory.push(AppUsageMemory {
            wss_bytes,
            pss_bytes: rollup.pss_bytes,
            ..sysinfo_memory(process.memory())
        });
    }
    // Without a successful reset, the next sample has no WSS and no window to report.
    if !baseline.start_times.is_empty() {
        baseline.reset_at = Some(Instant::now());
    }
    let working_set_window_ms = if measure_working_set {
        previous
            .reset_at
            .map(|reset_at| u64::try_from(reset_at.elapsed().as_millis()).unwrap_or(u64::MAX))
    } else {
        None
    };
    MemorySample {
        memory,
        working_set_window_ms,
    }
}

/// Clears the referenced and accessed bits on every page of the process.
/// See `proc_pid_clear_refs(5)`.
#[cfg(target_os = "linux")]
fn reset_referenced_pages(pid: Pid) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .open(format!("/proc/{pid}/clear_refs"))?
        .write_all(b"1")
}

#[derive(Debug, Default, PartialEq, Eq)]
// Linux smaps parsing is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) struct SmapsRollup {
    pub pss_bytes: Option<u64>,
    pub referenced_bytes: Option<u64>,
}

// Linux smaps parsing is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) fn parse_smaps_rollup(text: &str) -> SmapsRollup {
    let mut rollup = SmapsRollup::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let figure = match key {
            "Pss" => &mut rollup.pss_bytes,
            "Referenced" => &mut rollup.referenced_bytes,
            _ => continue,
        };
        *figure = parse_kib(value);
    }
    rollup
}

// Linux smaps parsing is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
fn parse_kib(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let kib = parts.next()?.parse::<u64>().ok()?;
    if parts.next() != Some("kB") || parts.next().is_some() {
        return None;
    }
    kib.checked_mul(1024)
}

#[cfg(test)]
mod tests {
    use super::{MemoryTotal, SmapsRollup, parse_smaps_rollup};
    use kubecove_models::AppUsageMemory;

    // Failure modes: a `Pss_*` line read as `Pss`, the `[rollup]` header read
    // as a field, a missing field read as zero, a unit other than kB, and a
    // kB value whose byte count overflows.
    #[test]
    fn parses_only_exact_smaps_rollup_fields() {
        let text = "\
00400000-7ffd5a9fe000 ---p 00000000 00:00 0                    [rollup]
Rss:              235268 kB
Pss:              220233 kB
Pss_Dirty:        187544 kB
Pss_Anon:         171976 kB
Referenced:        26460 kB
";
        assert_eq!(
            parse_smaps_rollup(text),
            SmapsRollup {
                pss_bytes: Some(220_233 * 1024),
                referenced_bytes: Some(26_460 * 1024),
            }
        );
        assert_eq!(
            parse_smaps_rollup("Pss_Anon: 12 kB\nReferenced: 4 MB\n"),
            SmapsRollup::default()
        );
        assert_eq!(
            parse_smaps_rollup(&format!("Pss: {} kB\n", u64::MAX)).pss_bytes,
            None
        );
    }

    #[test]
    fn totals_keep_a_figure_only_when_every_process_reports_it() {
        let mut total = MemoryTotal::default();
        total.add(AppUsageMemory {
            wss_bytes: Some(1),
            rss_bytes: Some(10),
            pss_bytes: Some(5),
        });
        total.add(AppUsageMemory {
            wss_bytes: None,
            rss_bytes: Some(20),
            pss_bytes: Some(7),
        });
        assert_eq!(
            total.total(),
            AppUsageMemory {
                wss_bytes: None,
                rss_bytes: Some(30),
                pss_bytes: Some(12),
            }
        );
        assert_eq!(MemoryTotal::default().total(), AppUsageMemory::default());
    }
}
