use chrono::{DateTime, TimeZone, Utc};

pub(crate) fn resource_age(creation_timestamp: Option<DateTime<Utc>>) -> String {
    match creation_timestamp {
        Some(t) => {
            let now = Utc::now();
            let duration = now.signed_duration_since(t);
            if duration.num_days() > 0 {
                format!("{}d", duration.num_days())
            } else if duration.num_hours() > 0 {
                format!("{}h", duration.num_hours())
            } else if duration.num_minutes() > 0 {
                format!("{}m", duration.num_minutes())
            } else {
                "<1m".to_string()
            }
        }
        None => "unknown".to_string(),
    }
}

pub(crate) fn k8s_timestamp_to_datetime(
    timestamp: &k8s_openapi::jiff::Timestamp,
) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(timestamp.as_second(), 0).single()
}

pub(crate) fn k8s_creation_timestamp_to_rfc3339(
    timestamp: &Option<k8s_openapi::apimachinery::pkg::apis::meta::v1::Time>,
) -> Option<String> {
    timestamp
        .as_ref()
        .and_then(|t| k8s_timestamp_to_datetime(&t.0))
        .map(|dt| dt.to_rfc3339())
}
