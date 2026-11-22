use k8s_openapi::{
    api::core::v1::{Event, EventSource, ObjectReference},
    apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, Time},
};
use serde::{Deserialize, Deserializer};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_projection_preserves_filtering_ordering_source_and_timestamp_fallbacks() {
        use serde_json::json;
        for timestamp in [
            "eventTime",
            "lastTimestamp",
            "firstTimestamp",
            "creationTimestamp",
        ] {
            for component in [Some("controller"), Some(""), None] {
                let mut value = json!({"apiVersion":"v1", "kind":"Event",
                    "metadata":{"name":"event", "namespace":"tenant", "managedFields":[{"fieldsV1":{"f:message":{}}}]},
                    "involvedObject":{"kind":"Pod", "name":"app", "namespace":"tenant", "uid":"uid"},
                    "count":3, "message":"failure", "reason":"Failed", "type":"Warning",
                    "source":{"component":"fallback"}, "reportingComponent":component});
                if timestamp == "creationTimestamp" {
                    value["metadata"][timestamp] = json!("2026-10-01T00:00:00Z");
                } else {
                    value[timestamp] = json!("2026-10-01T00:00:00Z");
                }
                let full: Event = serde_json::from_value(value.clone()).unwrap();
                let projected: EventProjection = serde_json::from_value(value).unwrap();
                let summarize = |event| {
                    super::super::events::resource_events_after_fetch_at(
                        vec![event],
                        "Pod",
                        "app",
                        Some("tenant"),
                        Some("uid"),
                        None,
                    )
                };
                assert_eq!(
                    serde_json::to_vec(&summarize(full)).unwrap(),
                    serde_json::to_vec(&summarize(projected.0)).unwrap()
                );
            }
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(from = "Fields")]
pub(crate) struct EventProjection(pub(crate) Event);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fields {
    #[serde(default, rename = "apiVersion", deserialize_with = "api_version")]
    _api_version: (),
    #[serde(default, rename = "kind", deserialize_with = "kind")]
    _kind: (),
    #[serde(
        default,
        deserialize_with = "kubecove_kube::commands::list_projection::metadata"
    )]
    metadata: ObjectMeta,
    involved_object: Option<ObjectReference>,
    event_time: Option<MicroTime>,
    first_timestamp: Option<Time>,
    last_timestamp: Option<Time>,
    count: Option<i32>,
    message: Option<String>,
    reason: Option<String>,
    reporting_component: Option<String>,
    source: Option<EventSource>,
    #[serde(rename = "type")]
    type_: Option<String>,
}

fn api_version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    if String::deserialize(deserializer)? == "v1" {
        Ok(())
    } else {
        Err(serde::de::Error::custom("invalid Event apiVersion"))
    }
}

fn kind<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    if String::deserialize(deserializer)? == "Event" {
        Ok(())
    } else {
        Err(serde::de::Error::custom("invalid Event kind"))
    }
}

impl From<Fields> for EventProjection {
    fn from(value: Fields) -> Self {
        Self(Event {
            metadata: value.metadata,
            involved_object: value.involved_object.unwrap_or_default(),
            event_time: value.event_time,
            first_timestamp: value.first_timestamp,
            last_timestamp: value.last_timestamp,
            count: value.count,
            message: value.message,
            reason: value.reason,
            reporting_component: value.reporting_component,
            source: value.source,
            type_: value.type_,
            ..Event::default()
        })
    }
}
