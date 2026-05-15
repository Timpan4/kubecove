use crate::commands::helpers::{
    base_resource_summary, fmt_ready, list_params, resource_age,
};
use crate::models::{AppError, ResourceSummary};
use chrono::{TimeZone, Utc};
use kube::{api::Api, config::KubeConfigOptions, Client};
use std::time::Instant;

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
            let pods = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            pods.iter()
                .map(|pod| {
                    let mut summary = base_resource_summary(
                        "Pod",
                        &cluster_context,
                        &pod.metadata,
                        resource_age(pod.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    );
                    // Pod phase and ready containers
                    if let Some(ref status) = pod.status {
                        summary.status = Some(status.phase.clone().unwrap_or_default());
                        let ready = status
                            .conditions
                            .as_ref()
                            .and_then(|conds| conds.iter().find(|c| c.type_ == "Ready"))
                            .map(|c| c.status.clone());
                        summary.ready = ready;
                        // Count restarts from container statuses
                        let restarts: i32 = status
                            .container_statuses
                            .as_ref()
                            .map(|cs| cs.iter().map(|c| c.restart_count).sum())
                            .unwrap_or(0);
                        if restarts > 0 {
                            summary.restarts = Some(restarts);
                        }
                    }
                    summary
                })
                .collect()
        }
        "Deployment" => {
            let api: Api<k8s_openapi::api::apps::v1::Deployment> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let deployments = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            deployments
                .iter()
                .map(|deploy| {
                    let mut summary = base_resource_summary(
                        "Deployment",
                        &cluster_context,
                        &deploy.metadata,
                        resource_age(deploy.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    );
                    if let Some(ref status) = deploy.status {
                        let available = status.available_replicas.unwrap_or(0);
                        let ready = status.ready_replicas.unwrap_or(0);
                        let desired = status.replicas.unwrap_or(0);
                        summary.ready = Some(format!("{}/{}", ready, desired));
                        if available > 0 || ready > 0 || desired > 0 {
                            summary.status = Some(format!("Available: {}", available));
                        }
                    }
                    summary
                })
                .collect()
        }
        "Service" => {
            let api: Api<k8s_openapi::api::core::v1::Service> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let services = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            services
                .iter()
                .map(|svc| {
                    base_resource_summary(
                        "Service",
                        &cluster_context,
                        &svc.metadata,
                        resource_age(svc.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    )
                })
                .collect()
        }
        "ConfigMap" => {
            let api: Api<k8s_openapi::api::core::v1::ConfigMap> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let configmaps = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            configmaps
                .iter()
                .map(|cm| {
                    base_resource_summary(
                        "ConfigMap",
                        &cluster_context,
                        &cm.metadata,
                        resource_age(cm.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    )
                })
                .collect()
        }
        "StatefulSet" => {
            let api: Api<k8s_openapi::api::apps::v1::StatefulSet> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let statefulsets = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            statefulsets
                .iter()
                .map(|ss| {
                    let mut summary = base_resource_summary(
                        "StatefulSet",
                        &cluster_context,
                        &ss.metadata,
                        resource_age(ss.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    );
                    if let Some(ref status) = ss.status {
                        summary.ready = Some(fmt_ready(status.ready_replicas, status.replicas));
                    }
                    summary
                })
                .collect()
        }
        "DaemonSet" => {
            let api: Api<k8s_openapi::api::apps::v1::DaemonSet> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let daemonsets = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            daemonsets
                .iter()
                .map(|ds| {
                    let mut summary = base_resource_summary(
                        "DaemonSet",
                        &cluster_context,
                        &ds.metadata,
                        resource_age(ds.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    );
                    if let Some(ref status) = ds.status {
                        let ready = status.number_ready;
                        let desired = status.desired_number_scheduled;
                        summary.ready = Some(format!("{}/{}", ready, desired));
                    }
                    summary
                })
                .collect()
        }
        "Ingress" => {
            let api: Api<k8s_openapi::api::networking::v1::Ingress> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let ingresses = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            ingresses
                .iter()
                .map(|ing| {
                    base_resource_summary(
                        "Ingress",
                        &cluster_context,
                        &ing.metadata,
                        resource_age(ing.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    )
                })
                .collect()
        }
        "Secret" => {
            let api: Api<k8s_openapi::api::core::v1::Secret> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let secrets = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            secrets
                .iter()
                .map(|sec| {
                    base_resource_summary(
                        "Secret",
                        &cluster_context,
                        &sec.metadata,
                        resource_age(sec.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    )
                })
                .collect()
        }
        "PersistentVolumeClaim" => {
            let api: Api<k8s_openapi::api::core::v1::PersistentVolumeClaim> =
                if let Some(ns) = &namespace {
                    Api::namespaced(client, ns)
                } else {
                    Api::all(client)
                };
            let pvcs = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            pvcs.iter()
                .map(|pvc| {
                    base_resource_summary(
                        "PersistentVolumeClaim",
                        &cluster_context,
                        &pvc.metadata,
                        resource_age(pvc.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    )
                })
                .collect()
        }
        "Job" => {
            let api: Api<k8s_openapi::api::batch::v1::Job> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let jobs = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            jobs.iter()
                .map(|job| {
                    let mut summary = base_resource_summary(
                        "Job",
                        &cluster_context,
                        &job.metadata,
                        resource_age(job.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    );
                    if let Some(ref status) = job.status {
                        let active = status.active.unwrap_or(0);
                        let failed = status.failed.unwrap_or(0);
                        let succeeded = status.succeeded.unwrap_or(0);
                        summary.status = if failed > 0 {
                            Some("Failed".to_string())
                        } else if succeeded > 0 {
                            Some("Complete".to_string())
                        } else if active > 0 {
                            Some("Active".to_string())
                        } else {
                            Some("Pending".to_string())
                        };
                        summary.ready = Some(format!(
                            "{}/{}",
                            succeeded,
                            job.spec.as_ref().and_then(|s| s.completions).unwrap_or(1)
                        ));
                    }
                    summary
                })
                .collect()
        }
        "CronJob" => {
            let api: Api<k8s_openapi::api::batch::v1::CronJob> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let cronjobs = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            cronjobs
                .iter()
                .map(|cj| {
                    let mut summary = base_resource_summary(
                        "CronJob",
                        &cluster_context,
                        &cj.metadata,
                        resource_age(cj.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    );
                    if let Some(ref status) = cj.status {
                        let active = status.active.as_ref().map(|a| a.len()).unwrap_or(0);
                        if active > 0 {
                            summary.status = Some(format!("{} active", active));
                        }
                    }
                    summary
                })
                .collect()
        }
        "Node" => {
            let api: Api<k8s_openapi::api::core::v1::Node> = Api::all(client);
            let nodes = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            nodes
                .iter()
                .map(|node| {
                    let mut summary = base_resource_summary(
                        "Node",
                        &cluster_context,
                        &node.metadata,
                        resource_age(node.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    );
                    if let Some(ref status) = node.status {
                        if let Some(ref conditions) = status.conditions {
                            let ready_count = conditions
                                .iter()
                                .filter(|c| c.type_ == "Ready" && c.status == "True")
                                .count();
                            summary.ready = Some(format!("{}/{}", ready_count, 1));
                            let ready = conditions
                                .iter()
                                .find(|c| c.type_ == "Ready")
                                .map(|c| c.status.clone());
                            summary.status = ready;
                        }
                    }
                    summary
                })
                .collect()
        }
        "StorageClass" => {
            let api: Api<k8s_openapi::api::storage::v1::StorageClass> = Api::all(client);
            let scs = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            scs.iter()
                .map(|sc| {
                    base_resource_summary(
                        "StorageClass",
                        &cluster_context,
                        &sc.metadata,
                        resource_age(sc.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    )
                })
                .collect()
        }
        "PersistentVolume" => {
            let api: Api<k8s_openapi::api::core::v1::PersistentVolume> = Api::all(client);
            let pvs = api
                .list(&list_params())
                .await
                .map_err(|e| AppError::kube(e.to_string()))?;
            pvs.iter()
                .map(|pv| {
                    let mut summary = base_resource_summary(
                        "PersistentVolume",
                        &cluster_context,
                        &pv.metadata,
                        resource_age(pv.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second(), 0)
                                .single()
                                .unwrap_or_else(Utc::now)
                        })),
                    );
                    summary.status = pv.status.as_ref().and_then(|s| s.phase.as_ref()).cloned();
                    summary
                })
                .collect()
        }
        _ => {
            return Err(AppError::new(
                format!("unsupported resource kind: {}", kind),
                "cluster",
            ))
        }
    };

    Ok(summaries)
}

#[tauri::command]
pub async fn list_resources(
    cluster_context: String,
    kind: String,
    namespace: Option<String>,
) -> Result<Vec<ResourceSummary>, AppError> {
    let started = Instant::now();
    let namespace_label = namespace.as_deref().unwrap_or("<all>");
    eprintln!(
        "[k8s-manager:backend] list_resources start context={} kind={} namespace={}",
        cluster_context, kind, namespace_label
    );
    let result =
        resources_summary_from(cluster_context.clone(), kind.clone(), namespace.clone()).await;
    match &result {
        Ok(rows) => eprintln!("[k8s-manager:backend] list_resources done context={} kind={} namespace={} rows={} ms={}", cluster_context, kind, namespace_label, rows.len(), started.elapsed().as_millis()),
        Err(err) => eprintln!("[k8s-manager:backend] list_resources error context={} kind={} namespace={} error_kind={} message={} ms={}", cluster_context, kind, namespace_label, err.kind, err.message, started.elapsed().as_millis()),
    }
    result
}
