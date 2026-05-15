use crate::commands::helpers::{
    extract_argo_app, extract_helm_release, extract_owner_ref, fetch_and_serialize,
    fetch_and_serialize_cluster, fmt_ready, k8s_creation_timestamp_to_rfc3339, redact_secret,
    resource_age,
};
use crate::models::{AppError, ResourceDetailsFull, ResourceSummary};
use chrono::{TimeZone, Utc};
use kube::{config::KubeConfigOptions, Client};
use std::time::Instant;

pub async fn resource_details_from(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<ResourceDetailsFull, AppError> {
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
            let (pod, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Pod>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            let metadata = serde_json::to_value(&pod.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = pod
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let summary = ResourceSummary {
                kind: "Pod".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(pod.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&pod.metadata.creation_timestamp),
                status: pod
                    .status
                    .as_ref()
                    .map(|s| s.phase.clone().unwrap_or_default()),
                ready: pod.status.as_ref().and_then(|s| {
                    s.conditions
                        .as_ref()
                        .and_then(|conds| conds.iter().find(|c| c.type_ == "Ready"))
                        .map(|c| c.status.clone())
                }),
                restarts: pod.status.as_ref().and_then(|s| {
                    let r: i32 = s
                        .container_statuses
                        .as_ref()
                        .map(|cs| cs.iter().map(|c| c.restart_count).sum())
                        .unwrap_or(0);
                    if r > 0 {
                        Some(r)
                    } else {
                        None
                    }
                }),
                owner_ref: extract_owner_ref(&pod.metadata),
                argo_app: extract_argo_app(&pod.metadata),
                helm_release: extract_helm_release(&pod.metadata),
            };
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "Deployment" => {
            let (deploy, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::Deployment>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            let metadata = serde_json::to_value(&deploy.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = deploy
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let mut summary = ResourceSummary {
                kind: "Deployment".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(deploy.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&deploy.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&deploy.metadata),
                argo_app: extract_argo_app(&deploy.metadata),
                helm_release: extract_helm_release(&deploy.metadata),
            };
            if let Some(ref s) = deploy.status {
                summary.ready = Some(format!(
                    "{}/{}",
                    s.ready_replicas.unwrap_or(0),
                    s.replicas.unwrap_or(0)
                ));
            }
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "Service" => {
            let (svc, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Service>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            let metadata = serde_json::to_value(&svc.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = svc
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let summary = ResourceSummary {
                kind: "Service".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(svc.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&svc.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&svc.metadata),
                argo_app: extract_argo_app(&svc.metadata),
                helm_release: extract_helm_release(&svc.metadata),
            };
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "ConfigMap" => {
            let (cm, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::ConfigMap>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            let metadata = serde_json::to_value(&cm.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let summary = ResourceSummary {
                kind: "ConfigMap".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(cm.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&cm.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&cm.metadata),
                argo_app: extract_argo_app(&cm.metadata),
                helm_release: extract_helm_release(&cm.metadata),
            };
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status: None,
            })
        }
        "StatefulSet" => {
            let (ss, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::StatefulSet>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            let metadata = serde_json::to_value(&ss.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = ss
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let mut summary = ResourceSummary {
                kind: "StatefulSet".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(ss.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&ss.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&ss.metadata),
                argo_app: extract_argo_app(&ss.metadata),
                helm_release: extract_helm_release(&ss.metadata),
            };
            if let Some(ref s) = ss.status {
                summary.ready = Some(fmt_ready(s.ready_replicas, s.replicas));
            }
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "DaemonSet" => {
            let (ds, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::DaemonSet>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            let metadata = serde_json::to_value(&ds.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = ds
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let mut summary = ResourceSummary {
                kind: "DaemonSet".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(ds.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&ds.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&ds.metadata),
                argo_app: extract_argo_app(&ds.metadata),
                helm_release: extract_helm_release(&ds.metadata),
            };
            if let Some(ref s) = ds.status {
                summary.ready = Some(format!("{}/{}", s.number_ready, s.desired_number_scheduled));
            }
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "Ingress" => {
            let (ing, yaml) = fetch_and_serialize::<k8s_openapi::api::networking::v1::Ingress>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            let metadata = serde_json::to_value(&ing.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = ing
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let summary = ResourceSummary {
                kind: "Ingress".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(ing.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&ing.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&ing.metadata),
                argo_app: extract_argo_app(&ing.metadata),
                helm_release: extract_helm_release(&ing.metadata),
            };
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
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
            let metadata = serde_json::to_value(&sec.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let summary = ResourceSummary {
                kind: "Secret".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(sec.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&sec.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&sec.metadata),
                argo_app: extract_argo_app(&sec.metadata),
                helm_release: extract_helm_release(&sec.metadata),
            };
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status: None,
            })
        }
        "PersistentVolumeClaim" => {
            let (pvc, yaml) = fetch_and_serialize::<
                k8s_openapi::api::core::v1::PersistentVolumeClaim,
            >(client, namespace.as_deref(), &name)
            .await?;
            let metadata = serde_json::to_value(&pvc.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = pvc
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let summary = ResourceSummary {
                kind: "PersistentVolumeClaim".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(pvc.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&pvc.metadata.creation_timestamp),
                status: status
                    .as_ref()
                    .and_then(|s| s.get("phase").and_then(|p| p.as_str().map(str::to_owned))),
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&pvc.metadata),
                argo_app: extract_argo_app(&pvc.metadata),
                helm_release: extract_helm_release(&pvc.metadata),
            };
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "Job" => {
            let (job, yaml) = fetch_and_serialize::<k8s_openapi::api::batch::v1::Job>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            let metadata = serde_json::to_value(&job.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = job
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let mut summary = ResourceSummary {
                kind: "Job".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(job.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&job.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&job.metadata),
                argo_app: extract_argo_app(&job.metadata),
                helm_release: extract_helm_release(&job.metadata),
            };
            if let Some(ref s) = job.status {
                summary.ready = Some(format!(
                    "{}/{}",
                    s.succeeded.unwrap_or(0),
                    job.spec
                        .as_ref()
                        .and_then(|spec| spec.completions)
                        .unwrap_or(1)
                ));
                summary.status = if s.failed.unwrap_or(0) > 0 {
                    Some("Failed".to_string())
                } else if s.succeeded.unwrap_or(0) > 0 {
                    Some("Complete".to_string())
                } else if s.active.unwrap_or(0) > 0 {
                    Some("Active".to_string())
                } else {
                    Some("Pending".to_string())
                };
            }
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "CronJob" => {
            let (cj, yaml) = fetch_and_serialize::<k8s_openapi::api::batch::v1::CronJob>(
                client,
                namespace.as_deref(),
                &name,
            )
            .await?;
            let metadata = serde_json::to_value(&cj.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = cj
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let mut summary = ResourceSummary {
                kind: "CronJob".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(cj.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&cj.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&cj.metadata),
                argo_app: extract_argo_app(&cj.metadata),
                helm_release: extract_helm_release(&cj.metadata),
            };
            if let Some(ref s) = cj.status {
                if let Some(ref active) = s.active {
                    if !active.is_empty() {
                        summary.status = Some(format!("{} active", active.len()));
                    }
                }
            }
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "Node" => {
            let (node, yaml) =
                fetch_and_serialize_cluster::<k8s_openapi::api::core::v1::Node>(client, &name)
                    .await?;
            let metadata = serde_json::to_value(&node.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = node
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let summary = ResourceSummary {
                kind: "Node".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: None,
                age: resource_age(node.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&node.metadata.creation_timestamp),
                status: node.status.as_ref().and_then(|s| {
                    s.conditions
                        .as_ref()?
                        .iter()
                        .find(|c| c.type_ == "Ready")
                        .map(|c| c.status.clone())
                }),
                ready: node.status.as_ref().and_then(|s| {
                    s.conditions.as_ref().map(|conds| {
                        format!(
                            "{}/{}",
                            conds
                                .iter()
                                .filter(|c| c.type_ == "Ready" && c.status == "True")
                                .count(),
                            1
                        )
                    })
                }),
                restarts: None,
                owner_ref: None,
                argo_app: None,
                helm_release: None,
            };
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "StorageClass" => {
            let (sc, yaml) = fetch_and_serialize_cluster::<
                k8s_openapi::api::storage::v1::StorageClass,
            >(client, &name)
            .await?;
            let metadata = serde_json::to_value(&sc.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let summary = ResourceSummary {
                kind: "StorageClass".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: None,
                age: resource_age(sc.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&sc.metadata.creation_timestamp),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: None,
                argo_app: None,
                helm_release: None,
            };
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status: None,
            })
        }
        "PersistentVolume" => {
            let (pv, yaml) = fetch_and_serialize_cluster::<
                k8s_openapi::api::core::v1::PersistentVolume,
            >(client, &name)
            .await?;
            let metadata = serde_json::to_value(&pv.metadata)
                .map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = pv
                .status
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok());
            let summary = ResourceSummary {
                kind: "PersistentVolume".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: None,
                age: resource_age(pv.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second(), 0)
                        .single()
                        .unwrap_or_else(Utc::now)
                })),
                created_at: k8s_creation_timestamp_to_rfc3339(&pv.metadata.creation_timestamp),
                status: pv.status.as_ref().and_then(|s| s.phase.as_ref()).cloned(),
                ready: None,
                restarts: None,
                owner_ref: None,
                argo_app: None,
                helm_release: None,
            };
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
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
) -> Result<ResourceDetailsFull, AppError> {
    let started = Instant::now();
    let namespace_label = namespace.as_deref().unwrap_or("<cluster>");
    eprintln!(
        "[k8s-manager:backend] get_resource_details start context={} kind={} namespace={} name={}",
        cluster_context, kind, namespace_label, name
    );
    let result = resource_details_from(
        cluster_context.clone(),
        kind.clone(),
        name.clone(),
        namespace.clone(),
    )
    .await;
    match &result {
        Ok(details) => eprintln!("[k8s-manager:backend] get_resource_details done context={} kind={} namespace={} name={} yaml_bytes={} status={} ms={}", cluster_context, kind, namespace_label, name, details.yaml.len(), details.status.is_some(), started.elapsed().as_millis()),
        Err(err) => eprintln!("[k8s-manager:backend] get_resource_details error context={} kind={} namespace={} name={} error_kind={} message={} ms={}", cluster_context, kind, namespace_label, name, err.kind, err.message, started.elapsed().as_millis()),
    }
    result
}
