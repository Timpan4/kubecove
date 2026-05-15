use crate::commands::helpers::{
    fetch_and_serialize, fetch_and_serialize_cluster, redact_secret,
};
use crate::models::AppError;
use kube::{config::KubeConfigOptions, Client};
use std::time::Instant;

pub async fn resource_yaml_from(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<String, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.clone()),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;

    match kind.as_str() {
        "Pod" => {
            let (_pod, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Pod>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            Ok(yaml)
        }
        "Deployment" => {
            let (_deploy, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::Deployment>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            Ok(yaml)
        }
        "Service" => {
            let (_svc, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Service>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            Ok(yaml)
        }
        "ConfigMap" => {
            let (_cm, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::ConfigMap>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            Ok(yaml)
        }
        "StatefulSet" => {
            let (_ss, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::StatefulSet>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            Ok(yaml)
        }
        "DaemonSet" => {
            let (_ds, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::DaemonSet>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            Ok(yaml)
        }
        "Ingress" => {
            let (_ing, yaml) = fetch_and_serialize::<k8s_openapi::api::networking::v1::Ingress>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            Ok(yaml)
        }
        "Secret" => {
            let (mut sec, _yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Secret>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            redact_secret(&mut sec);
            let yaml = serde_yaml::to_string(&sec)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            Ok(yaml)
        }
        "PersistentVolumeClaim" => {
            let (_pvc, yaml) = fetch_and_serialize::<
                k8s_openapi::api::core::v1::PersistentVolumeClaim,
            >(client, namespace.as_deref(), &name)
            .await?;
            Ok(yaml)
        }
        "Job" => {
            let (_job, yaml) = fetch_and_serialize::<k8s_openapi::api::batch::v1::Job>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            Ok(yaml)
        }
        "CronJob" => {
            let (_cj, yaml) = fetch_and_serialize::<k8s_openapi::api::batch::v1::CronJob>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            Ok(yaml)
        }
        "Node" => {
            let (_node, yaml) =
                fetch_and_serialize_cluster::<k8s_openapi::api::core::v1::Node>(client, &name)
                    .await?;
            Ok(yaml)
        }
        "StorageClass" => {
            let (_sc, yaml) = fetch_and_serialize_cluster::<
                k8s_openapi::api::storage::v1::StorageClass,
            >(client, &name)
            .await?;
            Ok(yaml)
        }
        "PersistentVolume" => {
            let (_pv, yaml) = fetch_and_serialize_cluster::<
                k8s_openapi::api::core::v1::PersistentVolume,
            >(client, &name)
            .await?;
            Ok(yaml)
        }
        _ => Err(AppError::new(
            format!("unsupported resource kind: {}", kind),
            "cluster",
        )),
    }
}

#[tauri::command]
pub async fn get_resource_yaml(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<String, AppError> {
    let started = Instant::now();
    let namespace_label = namespace.as_deref().unwrap_or("<cluster>");
    eprintln!(
        "[k8s-manager:backend] get_resource_yaml start context={} kind={} namespace={} name={}",
        cluster_context, kind, namespace_label, name
    );
    let result = resource_yaml_from(
        cluster_context.clone(),
        kind.clone(),
        name.clone(),
        namespace.clone(),
    )
    .await;
    match &result {
        Ok(yaml) => eprintln!("[k8s-manager:backend] get_resource_yaml done context={} kind={} namespace={} name={} bytes={} ms={}", cluster_context, kind, namespace_label, name, yaml.len(), started.elapsed().as_millis()),
        Err(err) => eprintln!("[k8s-manager:backend] get_resource_yaml error context={} kind={} namespace={} name={} error_kind={} message={} ms={}", cluster_context, kind, namespace_label, name, err.kind, err.message, started.elapsed().as_millis()),
    }
    result
}
