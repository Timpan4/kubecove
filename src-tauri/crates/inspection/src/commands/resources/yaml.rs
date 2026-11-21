use crate::commands::{
    BackendCancellationRegistry, ClusterAccess, FiniteRead, diagnostic_field,
    helpers::{
        fetch_cluster_resource, fetch_resource, redact_secret, resource_json_value,
        serialize_resource_value,
    },
};
use crate::models::AppErrorKind;
use crate::models::{AppError, YamlEncoding, YamlViewMode};
use crate::runtime::State;
use base64::{Engine as _, engine::general_purpose::STANDARD};
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
    let value = match kind.as_str() {
        "Pod" => resource_json_value(&fetch_resource::<Pod>(client, ns, name).await?),
        "Deployment" => resource_json_value(&fetch_resource::<Deployment>(client, ns, name).await?),
        "ReplicaSet" => resource_json_value(&fetch_resource::<ReplicaSet>(client, ns, name).await?),
        "Service" => resource_json_value(&fetch_resource::<Service>(client, ns, name).await?),
        "ConfigMap" => resource_json_value(&fetch_resource::<ConfigMap>(client, ns, name).await?),
        "StatefulSet" => {
            resource_json_value(&fetch_resource::<StatefulSet>(client, ns, name).await?)
        }
        "DaemonSet" => resource_json_value(&fetch_resource::<DaemonSet>(client, ns, name).await?),
        "Ingress" => resource_json_value(&fetch_resource::<Ingress>(client, ns, name).await?),
        "Secret" => redacted_secret_value(fetch_resource::<Secret>(client, ns, name).await?),
        "PersistentVolumeClaim" => {
            resource_json_value(&fetch_resource::<PersistentVolumeClaim>(client, ns, name).await?)
        }
        "Job" => resource_json_value(&fetch_resource::<Job>(client, ns, name).await?),
        "CronJob" => resource_json_value(&fetch_resource::<CronJob>(client, ns, name).await?),
        "Node" => resource_json_value(&fetch_cluster_resource::<Node>(client, name).await?),
        "StorageClass" => {
            resource_json_value(&fetch_cluster_resource::<StorageClass>(client, name).await?)
        }
        "PersistentVolume" => {
            resource_json_value(&fetch_cluster_resource::<PersistentVolume>(client, name).await?)
        }
        "CustomResourceDefinition" => resource_json_value(
            &fetch_cluster_resource::<CustomResourceDefinition>(client, name).await?,
        ),
        _ => Err(AppError::new(
            format!("unsupported resource kind: {kind}"),
            AppErrorKind::Cluster,
        )),
    }?;
    serialize_resource_value(value, mode, encoding)
}

fn redacted_secret_value(mut secret: Secret) -> Result<serde_json::Value, AppError> {
    redact_secret(&mut secret);
    resource_json_value(&secret)
}

#[allow(dead_code)]
pub fn serialize_secret_resource_document(
    secret: Secret,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    serialize_resource_value(redacted_secret_value(secret)?, mode, encoding)
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
