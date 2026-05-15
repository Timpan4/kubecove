use crate::models::AppError;
use k8s_openapi::{ClusterResourceScope, NamespaceResourceScope};
use kube::{
    api::{Api, Resource},
    Client,
};
use serde::{de::DeserializeOwned, Serialize};

pub(crate) fn redact_secret(secret: &mut k8s_openapi::api::core::v1::Secret) {
    if let Some(ref mut data) = secret.data {
        for value in data.values_mut() {
            *value = k8s_openapi::ByteString(b"REDACTED".to_vec());
        }
    }
    if let Some(ref mut string_data) = secret.string_data {
        for value in string_data.values_mut() {
            *value = "REDACTED".to_string();
        }
    }
}

pub(crate) async fn fetch_and_serialize<
    T: Resource<Scope = NamespaceResourceScope>
        + Serialize
        + DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
>(
    client: Client,
    namespace: Option<&str>,
    name: &str,
) -> Result<(T, String), AppError>
where
    <T as Resource>::DynamicType: Default,
{
    let api: Api<T> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    let resource = api
        .get(name)
        .await
        .map_err(|e: kube::Error| AppError::kube(e.to_string()))?;
    let yaml = serde_yaml::to_string(&resource)
        .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    Ok((resource, yaml))
}

pub(crate) async fn fetch_and_serialize_cluster<
    T: Resource<Scope = ClusterResourceScope>
        + Serialize
        + DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
>(
    client: Client,
    name: &str,
) -> Result<(T, String), AppError>
where
    <T as Resource>::DynamicType: Default,
{
    let api: Api<T> = Api::all(client);
    let resource = api
        .get(name)
        .await
        .map_err(|e: kube::Error| AppError::kube(e.to_string()))?;
    let yaml = serde_yaml::to_string(&resource)
        .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    Ok((resource, yaml))
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::Secret;
    use k8s_openapi::ByteString;
    use std::collections::BTreeMap;

    #[test]
    fn redacts_secret_data_and_string_data() {
        let mut data = BTreeMap::new();
        data.insert("password".to_string(), ByteString(b"super-secret".to_vec()));
        let mut string_data = BTreeMap::new();
        string_data.insert("token".to_string(), "plain-secret".to_string());
        let mut secret = Secret {
            data: Some(data),
            string_data: Some(string_data),
            ..Default::default()
        };

        redact_secret(&mut secret);

        assert_eq!(
            secret.data.as_ref().unwrap().get("password").unwrap().0,
            b"REDACTED".to_vec()
        );
        assert_eq!(
            secret.string_data.as_ref().unwrap().get("token").unwrap(),
            "REDACTED"
        );
    }
}
