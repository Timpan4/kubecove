use crate::commands::{
    diagnostic_field,
    helpers::{
        fetch_and_serialize_cluster_with_encoding, fetch_and_serialize_with_encoding,
        fetch_resource, redact_secret, serialize_resource_document,
    },
    BackendCancellationRegistry, ClusterAccess, FiniteRead,
};
use crate::models::AppErrorKind;
use crate::models::{AppError, YamlEncoding, YamlViewMode};
use crate::runtime::State;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{
    ConfigMap, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret, Service,
};
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::api::storage::v1::StorageClass;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::Api;

pub async fn reveal_secret_data_value(
    cluster_context: String,
    name: String,
    namespace: String,
    key: String,
    kubeconfig_env_var: Option<String>,
) -> Result<String, AppError> {
    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
    crate::commands::helpers::validate_namespace(Some(&namespace))?;
    crate::commands::helpers::validate_path_segment(&name, "Secret name")?;
    let secret = Api::<k8s_openapi::api::core::v1::Secret>::namespaced(
        access.read_client(&cluster_context).await?,
        &namespace,
    )
    .get(&name)
    .await
    .map_err(AppError::from)?;
    let value = secret
        .data
        .as_ref()
        .and_then(|data| data.get(&key))
        .ok_or_else(|| AppError::new("selected Secret key was not found", AppErrorKind::Cluster))?;
    Ok(STANDARD.encode(&value.0))
}

pub async fn resource_yaml_from(
    access: &ClusterAccess,
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
) -> Result<String, AppError> {
    let client = access.read_client(&cluster_context).await?;
    let mode = yaml_view_mode.unwrap_or_default();
    let encoding = yaml_encoding.unwrap_or_default();
    let ns = namespace.as_deref();
    let name = name.as_str();
    let yaml = match kind.as_str() {
        "Pod" => {
            fetch_and_serialize_with_encoding::<Pod>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "Deployment" => {
            fetch_and_serialize_with_encoding::<Deployment>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "ReplicaSet" => {
            fetch_and_serialize_with_encoding::<ReplicaSet>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "Service" => {
            fetch_and_serialize_with_encoding::<Service>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "ConfigMap" => {
            fetch_and_serialize_with_encoding::<ConfigMap>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "StatefulSet" => {
            fetch_and_serialize_with_encoding::<StatefulSet>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "DaemonSet" => {
            fetch_and_serialize_with_encoding::<DaemonSet>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "Ingress" => {
            fetch_and_serialize_with_encoding::<Ingress>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "Secret" => {
            let secret = fetch_resource::<Secret>(client, ns, name).await?;
            serialize_secret_resource_document(secret, mode, encoding)?
        }
        "PersistentVolumeClaim" => {
            fetch_and_serialize_with_encoding::<PersistentVolumeClaim>(
                client, ns, name, mode, encoding,
            )
            .await?
            .1
        }
        "Job" => {
            fetch_and_serialize_with_encoding::<Job>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "CronJob" => {
            fetch_and_serialize_with_encoding::<CronJob>(client, ns, name, mode, encoding)
                .await?
                .1
        }
        "Node" => {
            fetch_and_serialize_cluster_with_encoding::<Node>(client, name, mode, encoding)
                .await?
                .1
        }
        "StorageClass" => {
            fetch_and_serialize_cluster_with_encoding::<StorageClass>(client, name, mode, encoding)
                .await?
                .1
        }
        "PersistentVolume" => {
            fetch_and_serialize_cluster_with_encoding::<PersistentVolume>(
                client, name, mode, encoding,
            )
            .await?
            .1
        }
        "CustomResourceDefinition" => {
            fetch_and_serialize_cluster_with_encoding::<CustomResourceDefinition>(
                client, name, mode, encoding,
            )
            .await?
            .1
        }
        _ => {
            return Err(AppError::new(
                format!("unsupported resource kind: {kind}"),
                AppErrorKind::Cluster,
            ))
        }
    };
    Ok(yaml)
}

pub fn serialize_secret_resource_document(
    mut secret: k8s_openapi::api::core::v1::Secret,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    redact_secret(&mut secret);
    serialize_resource_document(&secret, mode, encoding)
}

pub async fn get_resource_yaml(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
    request_id: Option<String>,
    cancel_scope: Option<String>,
    cancellations: State<'_, BackendCancellationRegistry>,
) -> Result<String, AppError> {
    FiniteRead::new("get_resource_yaml")
        .label("context", &cluster_context)
        .label("kind", &kind)
        .label("namespace", namespace.as_deref().unwrap_or("<cluster>"))
        .label("name", &name)
        .cancellable(&cancellations, cancel_scope, request_id)
        .run(
            {
                let kind = kind.clone();
                async move {
                    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                    resource_yaml_from(
                        &access,
                        cluster_context,
                        kind,
                        name,
                        namespace,
                        yaml_view_mode,
                        yaml_encoding,
                    )
                    .await
                }
            },
            |yaml| {
                vec![
                    diagnostic_field("kind", &kind),
                    diagnostic_field("bytes", yaml.len()),
                ]
            },
        )
        .await
}
