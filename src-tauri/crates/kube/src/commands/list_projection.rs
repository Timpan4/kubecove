//! Internal list decoders. Detail/get paths continue to decode complete objects.
use futures_util::io::AsyncReadExt;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference, Time};
use kube::api::{Api, DynamicObject, ListParams, ObjectList, TypeMeta};
use serde::{de::DeserializeOwned, Deserialize, Deserializer};
use std::{collections::BTreeMap, future::Future, marker::PhantomData};

/// Bound list fetches without delaying new work behind an earlier slow result.
/// All fetches finish before callers inspect errors; dropping this future cancels
/// its children without spawning detached work.
pub async fn bounded_join_all<I>(futures: I, limit: usize) -> Vec<<I::Item as Future>::Output>
where
    I: IntoIterator,
    I::Item: Future,
{
    let permits = tokio::sync::Semaphore::new(limit);
    futures_util::future::join_all(futures.into_iter().map(|future| {
        let permits = &permits;
        async move {
            let _permit = permits.acquire().await.expect("list semaphore stays open");
            future.await
        }
    }))
    .await
}

pub async fn list<K: kube::Resource, P: DeserializeOwned + Clone>(
    api: Api<K>,
    params: &ListParams,
) -> Result<ObjectList<P>, kube::Error> {
    let mut request = kube::core::Request::new(api.resource_url())
        .list(params)
        .map_err(kube::Error::BuildRequest)?;
    request.extensions_mut().insert("list");
    let stream = api.into_client().request_stream(request).await?;
    futures_util::pin_mut!(stream);
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| kube::Error::Service(Box::new(error)))?;
    let text = String::from_utf8(bytes).map_err(kube::Error::FromUtf8)?;
    serde_json::from_str(&text).map_err(kube::Error::SerdeError)
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    name: Option<String>,
    namespace: Option<String>,
    uid: Option<String>,
    creation_timestamp: Option<Time>,
    deletion_timestamp: Option<Time>,
    generation: Option<i64>,
    labels: Option<BTreeMap<String, String>>,
    annotations: Option<Annotations>,
    owner_references: Option<Vec<OwnerReference>>,
}

struct Annotations(BTreeMap<String, String>);

impl<'de> Deserialize<'de> for Annotations {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Annotations;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("annotations")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut annotations = BTreeMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    if key == "kubectl.kubernetes.io/last-applied-configuration" {
                        map.next_value::<serde::de::IgnoredAny>()?;
                    } else {
                        annotations.insert(key, map.next_value::<String>()?);
                    }
                }
                Ok(Annotations(annotations))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

pub fn metadata<'de, D: Deserializer<'de>>(deserializer: D) -> Result<ObjectMeta, D::Error> {
    let value = Option::<Metadata>::deserialize(deserializer)?.unwrap_or_default();
    Ok(ObjectMeta {
        name: value.name,
        namespace: value.namespace,
        uid: value.uid,
        creation_timestamp: value.creation_timestamp,
        deletion_timestamp: value.deletion_timestamp,
        generation: value.generation,
        labels: value.labels,
        annotations: value.annotations.map(|value| value.0),
        owner_references: value.owner_references,
        ..ObjectMeta::default()
    })
}

#[derive(Deserialize)]
struct Fields<S, P> {
    #[serde(default, rename = "apiVersion", deserialize_with = "present_string")]
    api_version: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "metadata")]
    metadata: ObjectMeta,
    status: Option<S>,
    spec: Option<P>,
}

fn present_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

#[derive(Clone)]
pub struct Typed<K, S, P = serde::de::IgnoredAny> {
    pub metadata: ObjectMeta,
    pub status: Option<S>,
    pub spec: Option<P>,
    resource: PhantomData<K>,
}

impl<'de, K, S, P> Deserialize<'de> for Typed<K, S, P>
where
    K: k8s_openapi::Resource,
    S: Deserialize<'de>,
    P: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Fields::<S, P>::deserialize(deserializer)?;
        if value
            .api_version
            .as_deref()
            .is_some_and(|version| version != K::API_VERSION)
            || value.kind.as_deref().is_some_and(|kind| kind != K::KIND)
        {
            return Err(serde::de::Error::custom("invalid resource type metadata"));
        }
        Ok(Self {
            metadata: value.metadata,
            status: value.status,
            spec: value.spec,
            resource: PhantomData,
        })
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dynamic {
    api_version: Option<String>,
    kind: Option<String>,
    #[serde(default, deserialize_with = "metadata")]
    metadata: ObjectMeta,
    #[serde(default, deserialize_with = "present_value")]
    status: Option<serde_json::Value>,
}

fn present_value<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(deserializer).map(Some)
}

impl From<Dynamic> for DynamicObject {
    fn from(value: Dynamic) -> Self {
        let mut data = serde_json::Map::new();
        if let Some(status) = value.status {
            data.insert("status".into(), status);
        }
        Self {
            types: value
                .api_version
                .zip(value.kind)
                .map(|(api_version, kind)| TypeMeta { api_version, kind }),
            metadata: value.metadata,
            data: serde_json::Value::Object(data),
        }
    }
}

pub async fn list_dynamic(
    api: Api<DynamicObject>,
    params: &ListParams,
) -> Result<ObjectList<DynamicObject>, kube::Error> {
    let objects: ObjectList<Dynamic> = list(api, params).await?;
    Ok(ObjectList {
        types: objects.types,
        metadata: objects.metadata,
        items: objects.items.into_iter().map(Into::into).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;

    #[test]
    fn dynamic_status_preserves_missing_null_and_populated_values() {
        for json in [
            r#"{"apiVersion":"example.com/v1","kind":"Widget","metadata":{}}"#,
            r#"{"apiVersion":"example.com/v1","kind":"Widget","metadata":{},"status":null}"#,
            r#"{"apiVersion":"example.com/v1","kind":"Widget","metadata":{},"status":{"ready":true}}"#,
        ] {
            let full: DynamicObject = serde_json::from_str(json).unwrap();
            let projected: DynamicObject = serde_json::from_str::<Dynamic>(json).unwrap().into();
            assert_eq!(projected.data, full.data);
        }
    }

    #[tokio::test]
    async fn bounded_fetches_reuse_slots_and_preserve_order_after_errors() {
        let started = std::cell::Cell::new(0);
        let mut releases = Vec::new();
        let fetches = (0..18)
            .map(|index| {
                let (release, wait) = tokio::sync::oneshot::channel::<()>();
                releases.push(Some(release));
                let started = &started;
                async move {
                    started.set(started.get() + 1);
                    wait.await.unwrap();
                    if index < 2 {
                        Err(index)
                    } else {
                        Ok(index)
                    }
                }
            })
            .collect::<Vec<_>>();
        let joined = bounded_join_all(fetches, 16);
        futures_util::pin_mut!(joined);
        assert!(joined.as_mut().now_or_never().is_none());
        assert_eq!(started.get(), 16);
        // The first fetch stays pending. Finishing the second must admit another.
        releases[1].take().unwrap().send(()).unwrap();
        assert!(joined.as_mut().now_or_never().is_none());
        assert_eq!(started.get(), 17);
        for release in releases.into_iter().flatten() {
            release.send(()).unwrap();
        }
        let results = joined.await;
        assert_eq!(started.get(), 18);
        assert_eq!(
            results,
            (0..18)
                .map(|index| if index < 2 { Err(index) } else { Ok(index) })
                .collect::<Vec<_>>()
        );
        assert_eq!(results.into_iter().collect::<Result<Vec<_>, _>>(), Err(0));
    }
}
