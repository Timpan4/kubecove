use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageMetricsBreakdown {
    pub label: String,
    pub description: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub process_count: usize,
    pub children: Vec<AppUsageMetricsBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageMetrics {
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub process_count: usize,
    pub sampled_at: String,
    pub breakdown: Vec<AppUsageMetricsBreakdown>,
}
