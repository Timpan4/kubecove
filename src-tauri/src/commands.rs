use crate::models::{AppError, ClusterContext, NamespaceSummary, ResourceSummary, ResourceDetailsFull};
use chrono::{DateTime, TimeZone, Utc};
use kube::{
    api::Api,
    config::{KubeConfigOptions, Kubeconfig},
    Client, Resource,
};
use k8s_openapi::NamespaceResourceScope;
use serde::{de::DeserializeOwned, Serialize};

/// ARGO CD ANNOTATIONS AND LABEL KEYS
const ANNOTATION_ARGOCD_APP_NAME: &str = "argocd.argoproj.io/name";
const ANNOTATION_ARGOCD_TRACKING_ID: &str = "argocd.argoproj.io/tracking-id";
const LABEL_ARGOCD_APP_NAME: &str = "argocd.argoproj.io/application";
const LABEL_APP_KUBERNETES_IO_INSTANCE: &str = "app.kubernetes.io/instance";
const LABEL_HELM_RELEASE_NAME: &str = "helm.sh/release";

/// Extract the name of the first owner reference, if any.
fn extract_owner_ref(metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta) -> Option<String> {
    metadata
        .owner_references
        .as_ref()
        .and_then(|refs| refs.iter().next())
        .map(|r| r.name.clone())
}

/// Check labels and annotations for Argo CD application tracking signals.
fn extract_argo_app(metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta) -> Option<String> {
    // Try annotation: argocd.argoproj.io/name or argocd.argoproj.io/tracking-id
    if let Some(annotations) = &metadata.annotations {
        if let Some(name) = annotations.get(ANNOTATION_ARGOCD_APP_NAME) {
            return Some(name.clone());
        }
        if let Some(id) = annotations.get(ANNOTATION_ARGOCD_TRACKING_ID) {
            return Some(id.clone());
        }
    }
    // Try label: argocd.argoproj.io/application
    if let Some(labels) = &metadata.labels {
        if let Some(name) = labels.get(LABEL_ARGOCD_APP_NAME) {
            return Some(name.clone());
        }
        if let Some(instance) = labels.get(LABEL_APP_KUBERNETES_IO_INSTANCE) {
            return Some(instance.clone());
        }
    }
    None
}

/// Check labels for Helm release signals.
fn extract_helm_release(metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta) -> Option<String> {
    metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get(LABEL_HELM_RELEASE_NAME))
        .cloned()
}

/// Build a base ResourceSummary with all Milestone-2 metadata fields pre-populated
/// from common metadata (owner refs, Argo CD, Helm). Individual resource blocks
/// override the status-specific fields after calling this.
fn base_resource_summary(
    kind: &str,
    cluster: &str,
    metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta,
    age: String,
) -> ResourceSummary {
    ResourceSummary {
        kind: kind.to_string(),
        cluster: cluster.to_string(),
        name: metadata.name.clone().unwrap_or_default(),
        namespace: metadata.namespace.clone(),
        age,
        status: None,
        ready: None,
        restarts: None,
        owner_ref: extract_owner_ref(metadata),
        argo_app: extract_argo_app(metadata),
        helm_release: extract_helm_release(metadata),
    }
}

/// Helper to unwrap an Option<i32> for formatting, defaulting to 0.
fn opt_i32_to_str(opt: Option<i32>) -> String {
    opt.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string())
}

/// Helper to format any integer-like value for ready count display.
fn fmt_ready(ready: Option<i32>, desired: i32) -> String {
    format!("{}/{}", opt_i32_to_str(ready), desired)
}

/// Redact secret payload fields to prevent exposure to frontend.
fn redact_secret(secret: &mut k8s_openapi::api::core::v1::Secret) {
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

/// Fetch a namespaced resource and serialize it to YAML.
async fn fetch_and_serialize<T: Resource<Scope = NamespaceResourceScope> + Serialize + DeserializeOwned + Clone + std::fmt::Debug + Send + Sync>(
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
    let resource = api.get(name).await.map_err(|e: kube::Error| AppError::kube(e.to_string()))?;
    let yaml = serde_yaml::to_string(&resource).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
    Ok((resource, yaml))
}

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
                .map(|pod| {
                    let mut summary = base_resource_summary(
                        "Pod",
                        &cluster_context,
                        &pod.metadata,
                        resource_age(pod.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
            let deployments = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            deployments
                .iter()
                .map(|deploy| {
                    let mut summary = base_resource_summary(
                        "Deployment",
                        &cluster_context,
                        &deploy.metadata,
                        resource_age(deploy.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
            let services = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            services
                .iter()
                .map(|svc| {
                    base_resource_summary(
                        "Service",
                        &cluster_context,
                        &svc.metadata,
                        resource_age(svc.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
            let configmaps = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            configmaps
                .iter()
                .map(|cm| {
                    base_resource_summary(
                        "ConfigMap",
                        &cluster_context,
                        &cm.metadata,
                        resource_age(cm.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
            let statefulsets = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            statefulsets
                .iter()
                .map(|ss| {
                    let mut summary = base_resource_summary(
                        "StatefulSet",
                        &cluster_context,
                        &ss.metadata,
                        resource_age(ss.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
            let daemonsets = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            daemonsets
                .iter()
                .map(|ds| {
                    let mut summary = base_resource_summary(
                        "DaemonSet",
                        &cluster_context,
                        &ds.metadata,
                        resource_age(ds.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
            let ingresses = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            ingresses
                .iter()
                .map(|ing| {
                    base_resource_summary(
                        "Ingress",
                        &cluster_context,
                        &ing.metadata,
                        resource_age(ing.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
            let secrets = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            secrets
                .iter()
                .map(|sec| {
                    base_resource_summary(
                        "Secret",
                        &cluster_context,
                        &sec.metadata,
                        resource_age(sec.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                        })),
                    )
                })
                .collect()
        }
        "PersistentVolumeClaim" => {
            let api: Api<k8s_openapi::api::core::v1::PersistentVolumeClaim> = if let Some(ns) = &namespace {
                Api::namespaced(client, ns)
            } else {
                Api::all(client)
            };
            let pvcs = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            pvcs
                .iter()
                .map(|pvc| {
                    base_resource_summary(
                        "PersistentVolumeClaim",
                        &cluster_context,
                        &pvc.metadata,
                        resource_age(pvc.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
            let jobs = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            jobs
                .iter()
                .map(|job| {
                    let mut summary = base_resource_summary(
                        "Job",
                        &cluster_context,
                        &job.metadata,
                        resource_age(job.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
                        summary.ready = Some(format!("{}/{}", succeeded, job.spec.as_ref().and_then(|s| s.completions).unwrap_or(1)));
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
            let cronjobs = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
            cronjobs
                .iter()
                .map(|cj| {
                    let mut summary = base_resource_summary(
                        "CronJob",
                        &cluster_context,
                        &cj.metadata,
                        resource_age(cj.metadata.creation_timestamp.clone().map(|t| {
                            Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
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
            let (_pod, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Pod>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        "Deployment" => {
            let (_deploy, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::Deployment>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        "Service" => {
            let (_svc, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Service>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        "ConfigMap" => {
            let (_cm, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::ConfigMap>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        "StatefulSet" => {
            let (_ss, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::StatefulSet>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        "DaemonSet" => {
            let (_ds, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::DaemonSet>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        "Ingress" => {
            let (_ing, yaml) = fetch_and_serialize::<k8s_openapi::api::networking::v1::Ingress>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        "Secret" => {
            let (mut sec, _yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Secret>(client, namespace.as_deref(), &name).await?;
            redact_secret(&mut sec);
            let yaml = serde_yaml::to_string(&sec).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            Ok(yaml)
        }
        "PersistentVolumeClaim" => {
            let (_pvc, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::PersistentVolumeClaim>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        "Job" => {
            let (_job, yaml) = fetch_and_serialize::<k8s_openapi::api::batch::v1::Job>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        "CronJob" => {
            let (_cj, yaml) = fetch_and_serialize::<k8s_openapi::api::batch::v1::CronJob>(client, namespace.as_deref(), &name).await?;
            Ok(yaml)
        }
        _ => Err(AppError::new(format!("unsupported resource kind: {}", kind), "cluster")),
    }
}

#[tauri::command]
pub async fn get_resource_yaml(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<String, AppError> {
    resource_yaml_from(cluster_context, kind, name, namespace).await
}

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
            let (pod, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Pod>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&pod.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = pod.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten();
            let summary = ResourceSummary {
                kind: "Pod".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(pod.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
                status: pod.status.as_ref().map(|s| s.phase.clone().unwrap_or_default()),
                ready: pod.status.as_ref().and_then(|s| {
                    s.conditions.as_ref().and_then(|conds| conds.iter().find(|c| c.type_ == "Ready")).map(|c| c.status.clone())
                }),
                restarts: pod.status.as_ref().and_then(|s| {
                    let r: i32 = s.container_statuses.as_ref().map(|cs| cs.iter().map(|c| c.restart_count).sum()).unwrap_or(0);
                    if r > 0 { Some(r) } else { None }
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
            let (deploy, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::Deployment>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&deploy.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = deploy.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten();
            let mut summary = ResourceSummary {
                kind: "Deployment".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(deploy.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&deploy.metadata),
                argo_app: extract_argo_app(&deploy.metadata),
                helm_release: extract_helm_release(&deploy.metadata),
            };
            if let Some(ref s) = deploy.status {
                summary.ready = Some(format!("{}/{}", s.ready_replicas.unwrap_or(0), s.replicas.unwrap_or(0)));
            }
            Ok(ResourceDetailsFull {
                summary,
                yaml,
                metadata,
                status,
            })
        }
        "Service" => {
            let (svc, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Service>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&svc.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = svc.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten();
            let summary = ResourceSummary {
                kind: "Service".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(svc.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
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
            let (cm, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::ConfigMap>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&cm.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let summary = ResourceSummary {
                kind: "ConfigMap".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(cm.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
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
            let (ss, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::StatefulSet>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&ss.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = ss.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten();
            let mut summary = ResourceSummary {
                kind: "StatefulSet".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(ss.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
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
            let (ds, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::DaemonSet>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&ds.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = ds.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten();
            let mut summary = ResourceSummary {
                kind: "DaemonSet".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(ds.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
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
            let (ing, yaml) = fetch_and_serialize::<k8s_openapi::api::networking::v1::Ingress>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&ing.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = ing.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten();
            let summary = ResourceSummary {
                kind: "Ingress".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(ing.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
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
            let (mut sec, _yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::Secret>(client, namespace.as_deref(), &name).await?;
            redact_secret(&mut sec);
            let yaml = serde_yaml::to_string(&sec).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let metadata = serde_json::to_value(&sec.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let summary = ResourceSummary {
                kind: "Secret".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(sec.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
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
            let (pvc, yaml) = fetch_and_serialize::<k8s_openapi::api::core::v1::PersistentVolumeClaim>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&pvc.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = pvc.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten();
            let summary = ResourceSummary {
                kind: "PersistentVolumeClaim".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(pvc.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
                status: status.as_ref().and_then(|s| s.get("phase").map(|p| p.to_string())),
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
            let (job, yaml) = fetch_and_serialize::<k8s_openapi::api::batch::v1::Job>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&job.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = job.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten();
            let mut summary = ResourceSummary {
                kind: "Job".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(job.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
                status: None,
                ready: None,
                restarts: None,
                owner_ref: extract_owner_ref(&job.metadata),
                argo_app: extract_argo_app(&job.metadata),
                helm_release: extract_helm_release(&job.metadata),
            };
            if let Some(ref s) = job.status {
                summary.ready = Some(format!("{}/{}", s.succeeded.unwrap_or(0), job.spec.as_ref().and_then(|spec| spec.completions).unwrap_or(1)));
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
            let (cj, yaml) = fetch_and_serialize::<k8s_openapi::api::batch::v1::CronJob>(client, namespace.as_deref(), &name).await?;
            let metadata = serde_json::to_value(&cj.metadata).map_err(|e| AppError::new(e.to_string(), "serialization"))?;
            let status = cj.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten();
            let mut summary = ResourceSummary {
                kind: "CronJob".to_string(),
                cluster: cluster_context.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                age: resource_age(cj.metadata.creation_timestamp.clone().map(|t| {
                    Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
                })),
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
        _ => Err(AppError::new(format!("unsupported resource kind: {}", kind), "cluster")),
    }
}

#[tauri::command]
pub async fn get_resource_details(
    cluster_context: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<ResourceDetailsFull, AppError> {
    resource_details_from(cluster_context, kind, name, namespace).await
}