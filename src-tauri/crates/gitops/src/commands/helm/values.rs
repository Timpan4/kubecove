use crate::models::HelmValuesSummary;

pub(super) fn values_summary(config: Option<&serde_json::Value>) -> HelmValuesSummary {
    match config {
        Some(serde_json::Value::Object(values)) => {
            let mut top_level_keys: Vec<String> = values.keys().cloned().collect();
            top_level_keys.sort();
            HelmValuesSummary {
                has_values: !values.is_empty(),
                value_count: values.len(),
                top_level_keys,
            }
        }
        Some(serde_json::Value::Null) => HelmValuesSummary {
            has_values: false,
            value_count: 0,
            top_level_keys: Vec::new(),
        },
        Some(_) => HelmValuesSummary {
            has_values: true,
            value_count: 1,
            top_level_keys: Vec::new(),
        },
        None => HelmValuesSummary {
            has_values: false,
            value_count: 0,
            top_level_keys: Vec::new(),
        },
    }
}
