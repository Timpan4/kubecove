mod cluster;
mod core;
mod workloads;

use crate::commands::kubeconfig::KubeconfigSource;
use crate::models::{AppError, ResourceDetailsFull, YamlEncoding, YamlViewMode};
use std::time::Instant;

pub async fn resource_details_from(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
) -> Result<ResourceDetailsFull, AppError> {
    let source = KubeconfigSource::new(kubeconfig_env_var)?;
    let client = source.client_for_context(&cluster_context).await?;

    match kind.as_str() {
        "Pod" => core::pod_details(client, cluster_context, name, namespace).await,
        "Service" => core::service_details(client, cluster_context, name, namespace).await,
        "ConfigMap" => core::configmap_details(client, cluster_context, name, namespace).await,
        "Secret" => core::secret_details(client, cluster_context, name, namespace).await,
        "PersistentVolumeClaim" => {
            core::pvc_details(client, cluster_context, name, namespace).await
        }
        "Deployment" => {
            workloads::deployment_details(client, cluster_context, name, namespace).await
        }
        "StatefulSet" => {
            workloads::statefulset_details(client, cluster_context, name, namespace).await
        }
        "DaemonSet" => workloads::daemonset_details(client, cluster_context, name, namespace).await,
        "Ingress" => workloads::ingress_details(client, cluster_context, name, namespace).await,
        "Job" => workloads::job_details(client, cluster_context, name, namespace).await,
        "CronJob" => workloads::cronjob_details(client, cluster_context, name, namespace).await,
        "Node" => cluster::node_details(client, cluster_context, name, namespace).await,
        "StorageClass" => {
            cluster::storageclass_details(client, cluster_context, name, namespace).await
        }
        "PersistentVolume" => cluster::pv_details(client, cluster_context, name, namespace).await,
        _ => Err(AppError::new(
            format!("unsupported resource kind: {}", kind),
            "cluster",
        )),
    }
}

#[tauri::command]
pub async fn get_resource_details(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    kubeconfig_env_var: Option<String>,
    _yaml_view_mode: Option<YamlViewMode>,
    _yaml_encoding: Option<YamlEncoding>,
) -> Result<ResourceDetailsFull, AppError> {
    let started = Instant::now();
    let namespace_label = namespace.as_deref().unwrap_or("<cluster>");
    eprintln!(
        "[kubecove:backend] get_resource_details start context={} kind={} namespace={} name={}",
        cluster_context, kind, namespace_label, name
    );
    let result = resource_details_from(
        cluster_context.clone(),
        kind.clone(),
        name.clone(),
        namespace.clone(),
        kubeconfig_env_var,
    )
    .await;
    match &result {
        Ok(details) => eprintln!("[kubecove:backend] get_resource_details done context={} kind={} namespace={} name={} yaml_bytes={} status={} ms={}", cluster_context, kind, namespace_label, name, details.yaml.len(), details.status.is_some(), started.elapsed().as_millis()),
        Err(err) => eprintln!("[kubecove:backend] get_resource_details error context={} kind={} namespace={} name={} error_kind={} message={} ms={}", cluster_context, kind, namespace_label, name, err.kind, err.message, started.elapsed().as_millis()),
    }
    result
}
