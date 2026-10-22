use crate::commands::{
    diagnostic_field,
    helpers::{
        fetch_and_serialize_cluster_with_encoding, fetch_and_serialize_with_encoding,
        fetch_resource, redact_secret, serialize_resource_document,
    },
    kubeconfig::KubeconfigSource,
    BackendCancellationRegistry, FiniteRead,
};
use crate::models::AppErrorKind;
use crate::models::{AppError, YamlEncoding, YamlViewMode};
use crate::runtime::State;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use kube::Api;

pub async fn reveal_secret_data_value(
    cluster_context: String,
    name: String,
    namespace: String,
    key: String,
    kubeconfig_env_var: Option<String>,
) -> Result<String, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    crate::commands::helpers::validate_namespace(Some(&namespace))?;
    crate::commands::helpers::validate_path_segment(&name, "Secret name")?;
    let secret = Api::<k8s_openapi::api::core::v1::Secret>::namespaced(
        source.client_for_context(&cluster_context).await?,
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
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    yaml_view_mode: Option<YamlViewMode>,
    yaml_encoding: Option<YamlEncoding>,
) -> Result<String, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    let client = source.client_for_context(&cluster_context).await?;
    let mode = yaml_view_mode.unwrap_or_default();
    let encoding = yaml_encoding.unwrap_or_default();

    resource_yaml_with_client(client, kind, name, namespace, mode, encoding).await
}

pub(super) async fn resource_yaml_with_client(
    client: kube::Client,
    kind: String,
    name: String,
    namespace: Option<String>,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    match kind.as_str() {
        "Pod" => {
            let (_pod, yaml) =
                fetch_and_serialize_with_encoding::<k8s_openapi::api::core::v1::Pod>(
                    client,
                    namespace.as_deref(),
                    &name,
                    mode,
                    encoding,
                )
                .await?;
            Ok(yaml)
        }
        "Deployment" => {
            let (_deploy, yaml) = fetch_and_serialize_with_encoding::<
                k8s_openapi::api::apps::v1::Deployment,
            >(client, namespace.as_deref(), &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "ReplicaSet" => {
            let (_rs, yaml) = fetch_and_serialize_with_encoding::<
                k8s_openapi::api::apps::v1::ReplicaSet,
            >(client, namespace.as_deref(), &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "Service" => {
            let (_svc, yaml) = fetch_and_serialize_with_encoding::<
                k8s_openapi::api::core::v1::Service,
            >(client, namespace.as_deref(), &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "ConfigMap" => {
            let (_cm, yaml) = fetch_and_serialize_with_encoding::<
                k8s_openapi::api::core::v1::ConfigMap,
            >(client, namespace.as_deref(), &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "StatefulSet" => {
            let (_ss, yaml) = fetch_and_serialize_with_encoding::<
                k8s_openapi::api::apps::v1::StatefulSet,
            >(client, namespace.as_deref(), &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "DaemonSet" => {
            let (_ds, yaml) = fetch_and_serialize_with_encoding::<
                k8s_openapi::api::apps::v1::DaemonSet,
            >(client, namespace.as_deref(), &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "Ingress" => {
            let (_ing, yaml) = fetch_and_serialize_with_encoding::<
                k8s_openapi::api::networking::v1::Ingress,
            >(client, namespace.as_deref(), &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "Secret" => {
            let secret = fetch_resource::<k8s_openapi::api::core::v1::Secret>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            serialize_secret_resource_document(secret, mode, encoding)
        }
        "PersistentVolumeClaim" => {
            let (_pvc, yaml) = fetch_and_serialize_with_encoding::<
                k8s_openapi::api::core::v1::PersistentVolumeClaim,
            >(client, namespace.as_deref(), &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "Job" => {
            let (_job, yaml) =
                fetch_and_serialize_with_encoding::<k8s_openapi::api::batch::v1::Job>(
                    client,
                    namespace.as_deref(),
                    &name,
                    mode,
                    encoding,
                )
                .await?;
            Ok(yaml)
        }
        "CronJob" => {
            let (_cj, yaml) = fetch_and_serialize_with_encoding::<
                k8s_openapi::api::batch::v1::CronJob,
            >(client, namespace.as_deref(), &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "Node" => {
            let (_node, yaml) = fetch_and_serialize_cluster_with_encoding::<
                k8s_openapi::api::core::v1::Node,
            >(client, &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "StorageClass" => {
            let (_sc, yaml) = fetch_and_serialize_cluster_with_encoding::<
                k8s_openapi::api::storage::v1::StorageClass,
            >(client, &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "PersistentVolume" => {
            let (_pv, yaml) = fetch_and_serialize_cluster_with_encoding::<
                k8s_openapi::api::core::v1::PersistentVolume,
            >(client, &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        "CustomResourceDefinition" => {
            let (_crd, yaml) = fetch_and_serialize_cluster_with_encoding::<
                k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
            >(client, &name, mode, encoding)
            .await?;
            Ok(yaml)
        }
        _ => Err(AppError::new(
            format!("unsupported resource kind: {kind}"),
            AppErrorKind::Cluster,
        )),
    }
}

pub fn serialize_secret_resource_document(
    mut secret: k8s_openapi::api::core::v1::Secret,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    let _discarded_first_document = serialize_resource_document(&secret, mode, encoding)?;
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
            resource_yaml_from(
                cluster_context,
                kind.clone(),
                name,
                namespace,
                kubeconfig_env_var,
                yaml_view_mode,
                yaml_encoding,
            ),
            |yaml| {
                vec![
                    diagnostic_field("kind", &kind),
                    diagnostic_field("bytes", yaml.len()),
                ]
            },
        )
        .await
}
