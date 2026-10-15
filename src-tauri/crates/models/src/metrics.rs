use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceMetricsAvailabilityStatus {
    Available,
    Unavailable,
    Forbidden,
    NoSamples,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMetricsAvailability {
    pub status: ResourceMetricsAvailabilityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMetricSummary {
    pub kind: String,
    pub cluster: String,
    pub name: String,
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_millicores: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampled_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_pods: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMetricsSummary {
    pub cluster: String,
    pub availability: ResourceMetricsAvailability,
    pub pods: Vec<ResourceMetricSummary>,
    pub nodes: Vec<ResourceMetricSummary>,
    pub workloads: Vec<ResourceMetricSummary>,
    pub warnings: Vec<String>,
}
