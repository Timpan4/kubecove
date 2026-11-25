//! List responses need release identity and chart metadata, never values or manifests.
use super::{
    DecodedHelmChart, DecodedHelmInfo, HELM_STORAGE_CONFIGMAP, HELM_STORAGE_SECRET, ReleaseFields,
    configmap_release_data, decode_release, release_summary, secret_release_data,
};
use crate::commands::{ClusterAccess, helpers::list_params};
use crate::models::{AppError, AppErrorKind, HelmReleaseList, HelmReleaseSummary};
use futures_util::FutureExt;
use k8s_openapi::{
    NamespaceResourceScope,
    api::core::v1::{ConfigMap, Namespace, Secret},
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{
    Client,
    api::{Api, ListParams},
};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use std::collections::BTreeMap;

const HELM_OWNER_SELECTOR: &str = "owner=helm";

#[derive(Deserialize)]
struct ReleaseSummary {
    name: Option<String>,
    namespace: Option<String>,
    version: Option<i32>,
    info: Option<DecodedHelmInfo>,
    chart: Option<DecodedHelmChart>,
    // Validate the original string type without allocating its contents.
    #[serde(rename = "manifest")]
    _manifest: Option<DiscardedString>,
}

struct DiscardedString;

impl<'de> Deserialize<'de> for DiscardedString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = DiscardedString;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a string")
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
                Ok(DiscardedString)
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

fn summary(
    cluster: &str,
    kind: &str,
    metadata: &ObjectMeta,
    data: Option<&[u8]>,
) -> Result<HelmReleaseSummary, AppError> {
    let decoded = data.and_then(|data| decode_release::<ReleaseSummary>(data).ok());
    let fields = decoded.as_ref().map(|value| ReleaseFields {
        name: value.name.as_deref(),
        namespace: value.namespace.as_deref(),
        version: value.version,
        info: value.info.as_ref(),
        chart: value.chart.as_ref(),
    });
    release_summary(cluster, kind, metadata, fields)
}

pub(super) fn secret_list_summary(
    cluster: &str,
    secret: &Secret,
) -> Result<HelmReleaseSummary, AppError> {
    summary(
        cluster,
        HELM_STORAGE_SECRET,
        &secret.metadata,
        secret_release_data(secret),
    )
}

pub(super) fn configmap_list_summary(
    cluster: &str,
    config: &ConfigMap,
) -> Result<HelmReleaseSummary, AppError> {
    summary(
        cluster,
        HELM_STORAGE_CONFIGMAP,
        &config.metadata,
        configmap_release_data(config),
    )
}

pub async fn list_helm_releases(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
) -> Result<HelmReleaseList, AppError> {
    let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
    helm_releases_from(&access, &cluster_context, None).await
}

/// Releases in one namespace, which needs only namespaced storage list permission, or
/// in every namespace when `namespace` is `None`.
pub async fn helm_releases_from(
    access: &ClusterAccess,
    cluster_context: &str,
    namespace: Option<&str>,
) -> Result<HelmReleaseList, AppError> {
    let client = access.read_client(cluster_context).await?;
    match namespace {
        Some(namespace) => helm_releases_in(client, cluster_context, namespace).await,
        None => all_helm_releases(client, cluster_context).await,
    }
}

async fn helm_releases_in(
    client: Client,
    cluster_context: &str,
    namespace: &str,
) -> Result<HelmReleaseList, AppError> {
    crate::commands::helpers::validate_namespace(Some(namespace))?;
    let namespaces = [namespace.to_owned()];
    let (secrets, configmaps) = futures_util::join!(
        Box::pin(list_releases_by_namespace::<Secret>(
            client.clone(),
            cluster_context,
            &namespaces,
            secret_list_summary,
        )),
        Box::pin(list_releases_by_namespace::<ConfigMap>(
            client,
            cluster_context,
            &namespaces,
            configmap_list_summary,
        )),
    );
    let mut summaries = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for (storage_kind, listed) in [
        (HELM_STORAGE_SECRET, secrets),
        (HELM_STORAGE_CONFIGMAP, configmaps),
    ] {
        match listed {
            Ok((mut releases, _)) => summaries.append(&mut releases),
            Err(err) => {
                warnings.push(format!(
                    "Helm {storage_kind} storage unavailable: {}.",
                    storage_reason(&err)
                ));
                errors.push(err);
            }
        }
    }
    if errors.len() == 2 {
        return Err(storage_errors(errors));
    }
    Ok(HelmReleaseList {
        releases: latest_releases(summaries),
        warnings,
    })
}

fn storage_reason(err: &AppError) -> &str {
    if err.kind == AppErrorKind::Forbidden {
        "forbidden by RBAC"
    } else {
        &err.message
    }
}

async fn all_helm_releases(
    client: Client,
    cluster_context: &str,
) -> Result<HelmReleaseList, AppError> {
    // Both storage kinds and the namespace list are read together. Only a
    // per-namespace fallback waits for the namespaces. The boxes keep this
    // future small while it holds both listings.
    let fallback = helm_fallback_namespaces(client.clone(), client.default_namespace()).shared();
    let (secrets, configmaps, (fallback_namespaces, namespaces_discovered)) = futures_util::join!(
        Box::pin(list_releases::<Secret>(
            client.clone(),
            cluster_context,
            fallback.clone(),
            secret_list_summary,
        )),
        Box::pin(list_releases::<ConfigMap>(
            client.clone(),
            cluster_context,
            fallback.clone(),
            configmap_list_summary,
        )),
        fallback,
    );
    let mut summaries = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut fell_back = false;
    let mut readable_storage = 0;

    for (storage_kind, listed) in [
        (HELM_STORAGE_SECRET, secrets),
        (HELM_STORAGE_CONFIGMAP, configmaps),
    ] {
        match listed {
            Ok((mut releases, skipped_namespaces)) => {
                readable_storage += 1;
                summaries.append(&mut releases);
                let Some(skipped_namespaces) = skipped_namespaces else {
                    continue;
                };
                fell_back = true;
                if !skipped_namespaces.is_empty() {
                    warnings.push(format!(
                        "Helm {storage_kind} storage unavailable in namespaces: {}.",
                        skipped_namespaces.join(", ")
                    ));
                }
            }
            Err(err) => {
                warnings.push(format!(
                    "Helm {storage_kind} storage unavailable: {}.",
                    storage_reason(&err)
                ));
                errors.push(err);
            }
        }
    }

    if fell_back && !namespaces_discovered {
        warnings.push(format!(
            "Namespaces could not be listed; Helm storage was checked only in: {}.",
            if fallback_namespaces.is_empty() {
                "no namespaces".to_string()
            } else {
                fallback_namespaces.join(", ")
            }
        ));
    }

    if readable_storage == 0 && !errors.is_empty() {
        return Err(storage_errors(errors));
    }

    Ok(HelmReleaseList {
        releases: latest_releases(summaries),
        warnings,
    })
}

/// Namespaces for the per-namespace fallback, and whether they were discovered.
async fn helm_fallback_namespaces(client: Client, default_namespace: &str) -> (Vec<String>, bool) {
    let api: Api<Namespace> = Api::all(client);
    match api.list(&list_params()).await {
        Ok(items) => {
            let namespaces: Vec<String> = items
                .items
                .into_iter()
                .filter_map(|namespace| namespace.metadata.name)
                .collect();
            if namespaces.is_empty() && !default_namespace.is_empty() {
                (vec![default_namespace.to_string()], true)
            } else {
                (namespaces, true)
            }
        }
        Err(_) if !default_namespace.is_empty() => (vec![default_namespace.to_string()], false),
        Err(_) => (Vec::new(), false),
    }
}

/// Release summaries plus namespaces a partially successful fallback could not read.
type NamespaceListing = (Vec<HelmReleaseSummary>, Vec<String>);

/// Release summaries plus the skipped namespaces if the cluster-wide list failed.
type StorageListing = (Vec<HelmReleaseSummary>, Option<Vec<String>>);

/// Builds a release summary from a storage Secret or `ConfigMap`.
type StorageSummaryFn<K> = fn(&str, &K) -> Result<HelmReleaseSummary, AppError>;

/// Lists one storage kind cluster-wide, falling back to the namespaces from
/// `fallback` when that fails.
async fn list_releases<K>(
    client: Client,
    cluster_context: &str,
    fallback: impl Future<Output = (Vec<String>, bool)>,
    to_summary: StorageSummaryFn<K>,
) -> Result<StorageListing, AppError>
where
    K: kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + Clone
        + DeserializeOwned
        + std::fmt::Debug,
{
    let api: Api<K> = Api::all(client.clone());
    let params = helm_list_params();
    let items = match api.list(&params).await {
        Ok(items) => items,
        Err(all_error) => {
            let (fallback_namespaces, _) = fallback.await;
            if fallback_namespaces.is_empty() {
                return Err(AppError::from(all_error));
            }
            return list_releases_by_namespace(
                client,
                cluster_context,
                &fallback_namespaces,
                to_summary,
            )
            .await
            .map(|(summaries, skipped)| (summaries, Some(skipped)))
            .map_err(|namespace_error| {
                storage_errors(vec![AppError::from(all_error), namespace_error])
            });
        }
    };
    let summaries = items
        .items
        .iter()
        .map(|object| to_summary(cluster_context, object))
        .collect::<Result<_, _>>()?;
    Ok((summaries, None))
}

async fn list_releases_by_namespace<K>(
    client: Client,
    cluster_context: &str,
    namespaces: &[String],
    to_summary: StorageSummaryFn<K>,
) -> Result<NamespaceListing, AppError>
where
    K: kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + Clone
        + DeserializeOwned
        + std::fmt::Debug,
{
    let params = helm_list_params();
    let mut summaries = Vec::new();
    let mut succeeded = false;
    let mut errors = Vec::new();
    let mut skipped = Vec::new();

    for namespace in namespaces {
        let api: Api<K> = Api::namespaced(client.clone(), namespace);
        match api.list(&params).await {
            Ok(items) => {
                succeeded = true;
                for object in &items.items {
                    summaries.push(to_summary(cluster_context, object)?);
                }
            }
            Err(err) => {
                let mut error = AppError::from(err);
                error.message = format!("{namespace}: {}", error.message);
                errors.push(error);
                skipped.push(namespace.clone());
            }
        }
    }

    if !succeeded && !errors.is_empty() {
        Err(storage_errors(errors))
    } else {
        Ok((summaries, skipped))
    }
}

#[derive(Debug)]
struct StorageErrors(Vec<AppError>);

impl std::fmt::Display for StorageErrors {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, error) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str("; ")?;
            }
            std::fmt::Display::fmt(error, formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for StorageErrors {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.first().map(|error| error as &dyn std::error::Error)
    }
}

fn storage_errors(errors: Vec<AppError>) -> AppError {
    let kind = errors
        .first()
        .map_or(AppErrorKind::Cluster, |error| error.kind);
    let kind = if errors.iter().all(|error| error.kind == kind) {
        kind
    } else {
        AppErrorKind::Cluster
    };
    let source = StorageErrors(errors);
    AppError::new(source.to_string(), kind).with_source(source)
}

fn helm_list_params() -> ListParams {
    list_params().labels(HELM_OWNER_SELECTOR)
}

fn latest_releases(releases: Vec<HelmReleaseSummary>) -> Vec<HelmReleaseSummary> {
    let mut latest: BTreeMap<(String, String), HelmReleaseSummary> = BTreeMap::new();
    for release in releases {
        let key = (release.namespace.clone(), release.name.clone());
        let replace = latest
            .get(&key)
            .is_none_or(|existing| release.revision.unwrap_or(0) > existing.revision.unwrap_or(0));
        if replace {
            latest.insert(key, release);
        }
    }
    latest.into_values().collect()
}

#[cfg(feature = "bench-support")]
pub fn benchmark_storage_records(
    secrets: Vec<Secret>,
    configmaps: Vec<ConfigMap>,
) -> Result<Vec<HelmReleaseSummary>, AppError> {
    let mut summaries = Vec::with_capacity(secrets.len() + configmaps.len());
    for secret in secrets {
        summaries.push(secret_list_summary("benchmark", &secret)?);
    }
    for configmap in configmaps {
        summaries.push(configmap_list_summary("benchmark", &configmap)?);
    }
    Ok(latest_releases(summaries))
}
