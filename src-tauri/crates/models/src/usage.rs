use serde::{Deserialize, Serialize};

/// Memory figures for one process or a group of processes. A figure is `None`
/// when the operating system does not report it, when WSS has no baseline yet,
/// or when any process in a group lacks it, so a partial sum is never shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageMemory {
    /// Working set size. Windows reports it on every sample. Linux reports the
    /// pages referenced since the previous sample when WSS measurement is on.
    pub wss_bytes: Option<u64>,
    /// Resident set size on Linux and macOS.
    pub rss_bytes: Option<u64>,
    /// Proportional set size on Linux.
    pub pss_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageMetricsBreakdown {
    pub label: String,
    pub description: String,
    pub cpu_percent: f32,
    pub memory: AppUsageMemory,
    pub process_count: usize,
    pub children: Vec<AppUsageMetricsBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageMetrics {
    pub cpu_percent: f32,
    pub memory: AppUsageMemory,
    /// Milliseconds since the previous Linux WSS sample reset the referenced
    /// bits. `None` when the WSS figures do not cover a measured interval.
    pub working_set_window_ms: Option<u64>,
    pub process_count: usize,
    pub sampled_at: String,
    pub breakdown: Vec<AppUsageMetricsBreakdown>,
}
