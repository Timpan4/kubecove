//! Formatting and identity from `src/lib/resource-metrics.ts`.
use kubecove_lib::models::{
    AppError, AppErrorKind, ResourceMetricSummary, ResourceMetricsAvailability,
    ResourceMetricsAvailabilityStatus,
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

/// Why a context's metrics are missing, or `None` when they are available.
pub fn issue(value: &ResourceMetricsAvailability) -> Option<&str> {
    match value.status {
        ResourceMetricsAvailabilityStatus::Available => None,
        ResourceMetricsAvailabilityStatus::Forbidden => Some("metrics API forbidden"),
        ResourceMetricsAvailabilityStatus::NoSamples => Some("no metrics samples yet"),
        ResourceMetricsAvailabilityStatus::Unavailable => Some(
            value
                .message
                .as_deref()
                .unwrap_or("metrics API unavailable"),
        ),
    }
}

/// Why a context's metrics read failed.
pub fn read_issue(error: &AppError) -> &'static str {
    if error.kind == AppErrorKind::Forbidden {
        "metrics API forbidden"
    } else {
        "metrics API unavailable"
    }
}
