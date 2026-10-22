use super::builtin_summary::{builtin_summary, BuiltinSummary};
use crate::commands::diagnostics::record_backend_result;
use crate::commands::helpers::{fetch_and_serialize, fetch_and_serialize_cluster, redact_secret};
use crate::commands::{
    diagnostic_field, kubeconfig::KubeconfigSource, BackendCancellationRegistry,
};
use crate::models::AppErrorKind;
use crate::models::{AppError, ResourceDetailsFull, YamlEncoding, YamlViewMode};
use crate::runtime::State;
use chrono::{DateTime, Utc};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{
    ConfigMap, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret, Service,
};
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::api::storage::v1::StorageClass;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use std::time::Instant;

#[cfg(test)]
mod tests {
    use super::*;
    use http::{Request, Response};
    use kube::{client::Body, Client};
    use serde_json::json;

    #[tokio::test]
    async fn replicaset_details_load_from_namespaced_api() {
        for status in [
            Some(json!({"replicas": 3, "readyReplicas": 2, "availableReplicas": 1})),
            None,
        ] {
            let object = json!({
                "apiVersion": "apps/v1",
                "kind": "ReplicaSet",
                "metadata": {
                    "name": "calagopus-5f554d575f",
                    "namespace": "calagopus",
                    "ownerReferences": [{
                        "apiVersion": "apps/v1", "kind": "Deployment",
                        "name": "calagopus", "uid": "deployment-uid", "controller": true
                    }]
                },
                "status": status
            });
            let service = tower::service_fn(move |request: Request<Body>| {
                assert_eq!(request.method(), http::Method::GET);
                assert_eq!(
                    request.uri().path(),
                    "/apis/apps/v1/namespaces/calagopus/replicasets/calagopus-5f554d575f"
                );
                let body = object.to_string().into_bytes();
                async move { Ok::<_, std::convert::Infallible>(Response::new(Body::from(body))) }
            });
            let client = Client::new(service, "default");
            let details = resource_details_with_client(
                client.clone(),
                "test-cluster".into(),
                "ReplicaSet".into(),
                "calagopus-5f554d575f".into(),
                Some("calagopus".into()),
            )
            .await
            .expect("ReplicaSet details should load");
            assert_eq!(details.summary.kind, "ReplicaSet");
            assert_eq!(details.summary.cluster, "test-cluster");
            assert_eq!(details.summary.name, "calagopus-5f554d575f");
            assert_eq!(details.summary.namespace.as_deref(), Some("calagopus"));
            assert_eq!(details.summary.owner_ref.as_deref(), Some("calagopus"));
            assert_eq!(
                details.summary.ready.as_deref(),
                status.as_ref().map(|_| "2/3")
            );
            assert_eq!(
                details.summary.status.as_deref(),
                status.as_ref().map(|_| "Available: 1")
            );
            assert_eq!(details.status, status);
            assert_eq!(details.metadata["name"], "calagopus-5f554d575f");
            let yaml: serde_json::Value = serde_yaml::from_str(&details.yaml).unwrap();
            assert_eq!(yaml["kind"], "ReplicaSet");
            let document = super::super::yaml::resource_yaml_with_client(
                client,
                "ReplicaSet".into(),
                "calagopus-5f554d575f".into(),
                Some("calagopus".into()),
                YamlViewMode::default(),
                YamlEncoding::default(),
            )
            .await
            .expect("ReplicaSet YAML should load");
            let document: serde_json::Value = serde_yaml::from_str(&document).unwrap();
            assert_eq!(document, yaml);
        }
    }
}

pub async fn resource_details_from(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
) -> Result<ResourceDetailsFull, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    let client = source.client_for_context(&cluster_context).await?;

    resource_details_with_client(client, cluster_context, kind, name, namespace).await
}

pub(super) async fn resource_details_with_client(
    client: kube::Client,
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<ResourceDetailsFull, AppError> {
    let cluster = cluster_context.as_str();
    let namespace = namespace.as_deref();
    match kind.as_str() {
        "Pod" => namespaced_details::<Pod>(client, cluster, &name, namespace).await,
        "Service" => namespaced_details::<Service>(client, cluster, &name, namespace).await,
        "ConfigMap" => namespaced_details::<ConfigMap>(client, cluster, &name, namespace).await,
        "Secret" => {
            let (secret, _discarded_yaml) =
                fetch_and_serialize::<Secret>(client, namespace, &name).await?;
            secret_details_from_fetched(cluster, secret, Utc::now())
        }
        "PersistentVolumeClaim" => {
            namespaced_details::<PersistentVolumeClaim>(client, cluster, &name, namespace).await
        }
        "Deployment" => namespaced_details::<Deployment>(client, cluster, &name, namespace).await,
        "ReplicaSet" => namespaced_details::<ReplicaSet>(client, cluster, &name, namespace).await,
        "StatefulSet" => namespaced_details::<StatefulSet>(client, cluster, &name, namespace).await,
        "DaemonSet" => namespaced_details::<DaemonSet>(client, cluster, &name, namespace).await,
        "Ingress" => namespaced_details::<Ingress>(client, cluster, &name, namespace).await,
        "Job" => namespaced_details::<Job>(client, cluster, &name, namespace).await,
        "CronJob" => namespaced_details::<CronJob>(client, cluster, &name, namespace).await,
        "Node" => cluster_details::<Node>(client, cluster, &name).await,
        "StorageClass" => cluster_details::<StorageClass>(client, cluster, &name).await,
        "PersistentVolume" => cluster_details::<PersistentVolume>(client, cluster, &name).await,
        "CustomResourceDefinition" => {
            cluster_details::<CustomResourceDefinition>(client, cluster, &name).await
        }
        _ => Err(AppError::new(
            format!("unsupported resource kind: {kind}"),
            AppErrorKind::Cluster,
        )),
    }
}

async fn namespaced_details<K>(
    client: kube::Client,
    cluster_context: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<ResourceDetailsFull, AppError>
where
    K: BuiltinSummary
        + kube::Resource<DynamicType = (), Scope = k8s_openapi::NamespaceResourceScope>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
{
    let (object, yaml) = fetch_and_serialize::<K>(client, namespace, name).await?;
    details_from_fetched(cluster_context, &object, yaml, Utc::now())
}

async fn cluster_details<K>(
    client: kube::Client,
    cluster_context: &str,
    name: &str,
) -> Result<ResourceDetailsFull, AppError>
where
    K: BuiltinSummary
        + kube::Resource<DynamicType = (), Scope = k8s_openapi::ClusterResourceScope>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
{
    let (object, yaml) = fetch_and_serialize_cluster::<K>(client, name).await?;
    details_from_fetched(cluster_context, &object, yaml, Utc::now())
}

/// Details add the YAML document, raw metadata and raw status to the shared summary.
pub fn details_from_fetched<K: BuiltinSummary>(
    cluster_context: &str,
    object: &K,
    yaml: String,
    now: DateTime<Utc>,
) -> Result<ResourceDetailsFull, AppError> {
    let serialization = |e: serde_json::Error| {
        AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e)
    };
    Ok(ResourceDetailsFull {
        summary: builtin_summary(cluster_context, object, now),
        yaml,
        metadata: serde_json::to_value(object.metadata()).map_err(serialization)?,
        status: object.raw_status().transpose().map_err(serialization)?,
    })
}

/// Secret details discard the fetched YAML and re-serialize the redacted object.
pub fn secret_details_from_fetched(
    cluster_context: &str,
    mut secret: Secret,
    now: DateTime<Utc>,
) -> Result<ResourceDetailsFull, AppError> {
    redact_secret(&mut secret);
    let yaml = serde_yaml::to_string(&secret)
        .map_err(|e| AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e))?;
    details_from_fetched(cluster_context, &secret, yaml, now)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn get_resource_details(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    _yaml_view_mode: Option<YamlViewMode>,
    _yaml_encoding: Option<YamlEncoding>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    cancellations: State<'_, BackendCancellationRegistry>,
) -> Result<ResourceDetailsFull, AppError> {
    let started = Instant::now();
    let namespace_label = namespace.as_deref().unwrap_or("<cluster>");
    eprintln!(
        "[kubecove:backend] get_resource_details start context={cluster_context} kind={kind} namespace={namespace_label} name={name}"
    );
    let result = cancellations
        .execute(
            cancel_scope,
            request_id,
            resource_details_from(
                cluster_context.clone(),
                kind.clone(),
                name.clone(),
                namespace.clone(),
                kubeconfig_env_var,
            ),
        )
        .await;
    match &result {
        Ok(details) => {
            eprintln!("[kubecove:backend] get_resource_details done context={} kind={} namespace={} name={} yaml_bytes={} status={} ms={}", cluster_context, kind, namespace_label, name, details.yaml.len(), details.status.is_some(), started.elapsed().as_millis());
        }
        Err(err) if err.kind == AppErrorKind::Cancelled => {
            eprintln!("[kubecove:backend] get_resource_details cancelled context={} kind={} namespace={} name={} ms={}", cluster_context, kind, namespace_label, name, started.elapsed().as_millis());
        }
        Err(err) => {
            eprintln!("[kubecove:backend] get_resource_details error context={} kind={} namespace={} name={} error_kind={} message={} ms={}", cluster_context, kind, namespace_label, name, err.kind, err.message, started.elapsed().as_millis());
        }
    }
    record_backend_result("get_resource_details", started, &result, |details| {
        vec![
            diagnostic_field("kind", &kind),
            diagnostic_field("yamlBytes", details.yaml.len()),
            diagnostic_field("hasStatus", details.status.is_some()),
        ]
    });
    result
}
