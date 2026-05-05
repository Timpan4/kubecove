use crate::models::{AppError, ClusterContext, NamespaceSummary, ResourceSummary};
use chrono::{DateTime, TimeZone, Utc};
use kube::{
    api::Api,
    config::{KubeConfigOptions, Kubeconfig},
    Client,
};

pub fn get_cluster_contexts() -> Result<Vec<ClusterContext>, AppError> {
    let kubeconfig = Kubeconfig::read()
        .map_err(|e: kube::config::KubeconfigError| AppError::kube(e.to_string()))?;

    let cluster_contexts: Vec<ClusterContext> = kubeconfig
        .contexts
        .iter()
        .map(|ctx| ClusterContext {
            name: ctx.name.clone(),
        })
        .collect();

    Ok(cluster_contexts)
}

#[tauri::command]
pub fn list_kube_contexts() -> Result<Vec<ClusterContext>, AppError> {
    get_cluster_contexts()
}

fn namespace_age(creation_timestamp: Option<DateTime<Utc>>) -> String {
    match creation_timestamp {
        Some(t) => {
            let now = Utc::now();
            let duration = now.signed_duration_since(t);
            if duration.num_days() > 0 {
                format!("{}d", duration.num_days())
            } else if duration.num_hours() > 0 {
                format!("{}h", duration.num_hours())
            } else if duration.num_minutes() > 0 {
                format!("{}m", duration.num_minutes())
            } else {
                "<1m".to_string()
            }
        }
        None => "unknown".to_string(),
    }
}

pub async fn namespaces_summary_from(
    cluster_context: String,
) -> Result<Vec<NamespaceSummary>, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.clone()),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;
    let ns_api: Api<k8s_openapi::api::core::v1::Namespace> = Api::all(client);

    let namespaces = ns_api
        .list(&Default::default())
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let summaries: Vec<NamespaceSummary> = namespaces
        .iter()
        .map(|ns| NamespaceSummary {
            name: ns.metadata.name.clone().unwrap_or_default(),
            age: namespace_age(ns.metadata.creation_timestamp.clone().map(|t| {
                Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
            })),
        })
        .collect();

    Ok(summaries)
}

#[tauri::command]
pub async fn list_namespaces(cluster_context: String) -> Result<Vec<NamespaceSummary>, AppError> {
    namespaces_summary_from(cluster_context).await
}

fn resource_age(creation_timestamp: Option<DateTime<Utc>>) -> String {
    match creation_timestamp {
        Some(t) => {
            let now = Utc::now();
            let duration = now.signed_duration_since(t);
            if duration.num_days() > 0 {
                format!("{}d", duration.num_days())
            } else if duration.num_hours() > 0 {
                format!("{}h", duration.num_hours())
            } else if duration.num_minutes() > 0 {
                format!("{}m", duration.num_minutes())
            } else {
                "<1m".to_string()
            }
        }
        None => "unknown".to_string(),
    }
}

pub async fn resources_summary_from(
    cluster_context: String,
    kind: String,
    namespace: Option<String>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let options = KubeConfigOptions {
        context: Some(cluster_context.clone()),
        ..Default::default()
    };

    let config = kube::Config::from_kubeconfig(&options)
        .await
        .map_err(|e| AppError::kube(e.to_string()))?;

    let client = Client::try_from(config).map_err(|e| AppError::kube(e.to_string()))?;

    let summaries: Vec<ResourceSummary> = match kind.as_str() {
        "Pod" => {
            let api: Api<k8s_openapi::api::core::v1::Pod> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let pods = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            pods.iter()
                .map(|pod| ResourceSummary {
                    kind: "Pod".to_string(),
                    cluster: cluster_context.clone(),
                    name: pod.metadata.name.clone().unwrap_or_default(),
                    namespace: pod.metadata.namespace.clone(),
                    age: resource_age(pod.metadata.creation_timestamp.clone().map(|t| {
                        Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                    })),
                })
                .collect()
        }
        "Deployment" => {
            let api: Api<k8s_openapi::api::apps::v1::Deployment> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let deployments = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            deployments
                .iter()
                .map(|deploy| ResourceSummary {
                    kind: "Deployment".to_string(),
                    cluster: cluster_context.clone(),
                    name: deploy.metadata.name.clone().unwrap_or_default(),
                    namespace: deploy.metadata.namespace.clone(),
                    age: resource_age(deploy.metadata.creation_timestamp.clone().map(|t| {
                        Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                    })),
                })
                .collect()
        }
        "Service" => {
            let api: Api<k8s_openapi::api::core::v1::Service> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let services = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            services
                .iter()
                .map(|svc| ResourceSummary {
                    kind: "Service".to_string(),
                    cluster: cluster_context.clone(),
                    name: svc.metadata.name.clone().unwrap_or_default(),
                    namespace: svc.metadata.namespace.clone(),
                    age: resource_age(svc.metadata.creation_timestamp.clone().map(|t| {
                        Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                    })),
                })
                .collect()
        }
        "ConfigMap" => {
            let api: Api<k8s_openapi::api::core::v1::ConfigMap> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let configmaps = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            configmaps
                .iter()
                .map(|cm| ResourceSummary {
                    kind: "ConfigMap".to_string(),
                    cluster: cluster_context.clone(),
                    name: cm.metadata.name.clone().unwrap_or_default(),
                    namespace: cm.metadata.namespace.clone(),
                    age: resource_age(cm.metadata.creation_timestamp.clone().map(|t| {
                        Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                    })),
                })
                .collect()
        }
        _ => return Err(AppError::new(format!("unsupported resource kind: {}", kind), "cluster")),
    };

    Ok(summaries)
}

#[tauri::command]
pub async fn list_resources(
    cluster_context: String,
    kind: String,
    namespace: Option<String>,
) -> Result<Vec<ResourceSummary>, AppError> {
    resources_summary_from(cluster_context, kind, namespace).await
}