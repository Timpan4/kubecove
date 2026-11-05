//! Formatting and identity from `src/lib/resource-metrics.ts`.
use kubecove_lib::models::{
    ResourceMetricSummary, ResourceMetricsAvailability, ResourceMetricsAvailabilityStatus,
};

pub fn cpu(value: Option<f64>) -> String {
    match value {
        None => "—".into(),
        Some(value) if value >= 1000. => format!("{:.2} cores", value / 1000.),
        Some(value) if value >= 10. => format!("{:.0}m", value.round()),
        Some(value) => format!("{value:.1}m"),
    }
}

pub fn memory(value: Option<i64>) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    let units = ["B", "Ki", "Mi", "Gi", "Ti"];
    let mut current = value as f64;
    let mut index = 0;
    while current >= 1024. && index < units.len() - 1 {
        current /= 1024.;
        index += 1;
    }
    let precision = usize::from(index != 0 && current < 10.);
    format!("{current:.precision$} {}", units[index])
}

pub fn compact(metric: &ResourceMetricSummary) -> Option<String> {
    let mut parts = Vec::new();
    if metric.cpu_millicores.is_some() {
        parts.push(cpu(metric.cpu_millicores));
    }
    if metric.memory_bytes.is_some() {
        parts.push(memory(metric.memory_bytes));
    }
    (!parts.is_empty()).then(|| parts.join(" / "))
}

pub fn availability(value: &ResourceMetricsAvailability) -> &str {
    match value.status {
        ResourceMetricsAvailabilityStatus::Available => "metrics available",
        ResourceMetricsAvailabilityStatus::Forbidden => "metrics API forbidden",
        ResourceMetricsAvailabilityStatus::NoSamples => "no metrics samples yet",
        ResourceMetricsAvailabilityStatus::Unavailable => value
            .message
            .as_deref()
            .unwrap_or("metrics API unavailable"),
    }
}
