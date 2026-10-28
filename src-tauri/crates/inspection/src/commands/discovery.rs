use crate::models::AppErrorKind;
use crate::runtime::State;
use crate::{
    commands::helpers::list_params,
    commands::{diagnostic_field, ClusterAccess, ClusterLiveStore, FiniteRead},
    models::{AppError, DiscoveredResourceKind},
};
use futures_util::{io::AsyncReadExt, stream, StreamExt};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{
    api::{Api, ApiResource, DynamicObject},
    Client,
};
use std::sync::Arc;

mod crd_catalog;
#[cfg(feature = "bench-support")]
pub use crd_catalog::decode as decode_crd_catalog;
use crd_catalog::{CatalogCrd, CatalogVersion};

const MAX_PRESENT_KIND_CONCURRENCY: usize = 48;

pub async fn resource_kinds_from(
    access: &ClusterAccess,
    cluster_context: &str,
) -> Result<Vec<DiscoveredResourceKind>, AppError> {
    crd_resource_kinds(access.read_client(cluster_context).await?).await
}

pub(crate) async fn crd_resource_kinds(
    client: Client,
) -> Result<Vec<DiscoveredResourceKind>, AppError> {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    let mut request = kube::core::Request::new(api.resource_url())
        .list(&list_params())
        .map_err(kube::Error::BuildRequest)?;
    request.extensions_mut().insert("list");
    let stream = api
        .into_client()
        .request_stream(request)
        .await
        .map_err(AppError::from)?;
    futures_util::pin_mut!(stream);
    // Consume HTTP chunks as they arrive, then reuse the buffer for UTF-8.
    // request_text retains all chunks while collecting and copying the body.
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| kube::Error::Service(Box::new(error)))?;
    let text = String::from_utf8(bytes).map_err(kube::Error::FromUtf8)?;
    crd_catalog::decode(&text)
        .map_err(kube::Error::SerdeError)
        .map_err(AppError::from)
}

#[cfg(feature = "bench-support")]
pub fn crd_resource_kinds_from(
    definitions: impl IntoIterator<Item = CustomResourceDefinition>,
) -> Vec<DiscoveredResourceKind> {
    let mut kinds = definitions
        .into_iter()
        .filter_map(|crd| discovered_kind_from_crd(&CatalogCrd::from(crd)))
        .collect();
    sort_and_dedup_kinds(&mut kinds);
    kinds
}

fn discovered_kind_from_crd(crd: &CatalogCrd) -> Option<DiscoveredResourceKind> {
    let version = preferred_crd_version(&crd.spec.versions)?;
    let group = crd.spec.group.clone();
    let api_version = format!("{group}/{}", version.name);
    Some(DiscoveredResourceKind {
        group,
        version: version.name.clone(),
        api_version,
        kind: crd.spec.names.kind.clone(),
        plural: crd.spec.names.plural.clone(),
        short_names: crd.spec.names.short_names.clone().unwrap_or_default(),
        namespaced: crd.spec.scope == "Namespaced",
    })
}

fn preferred_crd_version(versions: &[CatalogVersion]) -> Option<&CatalogVersion> {
    versions
        .iter()
        .find(|version| version.served && version.storage)
        .or_else(|| versions.iter().find(|version| version.served))
}

fn api_resource_from_kind(resource_kind: &DiscoveredResourceKind) -> ApiResource {
    ApiResource {
        group: resource_kind.group.clone(),
        version: resource_kind.version.clone(),
        api_version: resource_kind.api_version.clone(),
        kind: resource_kind.kind.clone(),
        plural: resource_kind.plural.clone(),
    }
}

fn should_probe_present_kind(
    resource_kind: &DiscoveredResourceKind,
    namespaces: &[String],
) -> bool {
    resource_kind.namespaced || namespaces.is_empty()
}

pub fn sort_and_dedup_kinds(kinds: &mut Vec<DiscoveredResourceKind>) {
    kinds.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then(a.kind.cmp(&b.kind))
            .then(a.version.cmp(&b.version))
            .then(a.plural.cmp(&b.plural))
    });
    kinds.dedup_by(|a, b| {
        a.group == b.group && a.version == b.version && a.kind == b.kind && a.plural == b.plural
    });
}

pub async fn list_resource_kinds(
    cluster_context: String,
    kubeconfig_env_var: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<Arc<Vec<DiscoveredResourceKind>>, AppError> {
    FiniteRead::new("list_resource_kinds")
        .label("context", &cluster_context)
        .run(
            async move {
                let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                live_store
                    .resource_kinds(
                        access.source_key().to_string(),
                        cluster_context.clone(),
                        move || async move { resource_kinds_from(&access, &cluster_context).await },
                    )
                    .await
            },
            |rows| vec![diagnostic_field("rows", rows.len())],
        )
        .await
}

pub(crate) async fn present_custom_resource_kinds_for_catalog(
    client: Client,
    catalog: Vec<DiscoveredResourceKind>,
    namespaces: Vec<String>,
) -> Result<Vec<DiscoveredResourceKind>, AppError> {
    let namespaces: Vec<_> = namespaces
        .into_iter()
        .filter(|namespace| !namespace.trim().is_empty())
        .collect();

    for namespace in &namespaces {
        crate::commands::helpers::validate_namespace(Some(namespace))?;
    }

    let outcomes = stream::iter(
        catalog
            .into_iter()
            .filter(|kind| should_probe_present_kind(kind, &namespaces)),
    )
    .map(|resource_kind| {
        let client = client.clone();
        let namespaces = namespaces.clone();
        async move {
            match custom_resource_kind_present(client, &resource_kind, &namespaces).await {
                Ok(true) => Ok(Some(resource_kind)),
                Ok(false) => Ok(None),
                Err(err) if err.kind == AppErrorKind::NotFound => Ok(None),
                Err(err) if err.kind == AppErrorKind::Forbidden && !namespaces.is_empty() => {
                    Ok(None)
                }
                Err(err) => Err(err),
            }
        }
    })
    .buffer_unordered(MAX_PRESENT_KIND_CONCURRENCY)
    .collect::<Vec<Result<Option<DiscoveredResourceKind>, AppError>>>()
    .await;

    let mut present = Vec::new();
    for outcome in outcomes {
        if let Some(kind) = outcome? {
            present.push(kind);
        }
    }
    sort_and_dedup_kinds(&mut present);
    Ok(present)
}

async fn custom_resource_kind_present(
    client: Client,
    resource_kind: &DiscoveredResourceKind,
    namespaces: &[String],
) -> Result<bool, AppError> {
    let api_resource = api_resource_from_kind(resource_kind);
    crate::commands::helpers::validate_api_resource(&api_resource)?;
    let params = list_params().limit(1);
    if namespaces.is_empty() {
        let api: Api<DynamicObject> = Api::all_with(client, &api_resource);
        return api
            .list_metadata(&params)
            .await
            .map(|list| !list.items.is_empty())
            .map_err(AppError::from);
    }

    for namespace in namespaces {
        let api: Api<DynamicObject> =
            Api::namespaced_with(client.clone(), namespace, &api_resource);
        match api.list_metadata(&params).await {
            Ok(list) if !list.items.is_empty() => return Ok(true),
            Ok(_) => {}
            Err(err) => {
                let app_error = AppError::from(err);
                if app_error.kind == AppErrorKind::Forbidden
                    || app_error.kind == AppErrorKind::NotFound
                {
                    continue;
                }
                return Err(app_error);
            }
        }
    }
    Ok(false)
}

pub async fn list_present_custom_resource_kinds(
    cluster_context: String,
    namespaces: Vec<String>,
    kubeconfig_env_var: Option<String>,
    live_store: State<'_, ClusterLiveStore>,
) -> Result<Arc<Vec<DiscoveredResourceKind>>, AppError> {
    FiniteRead::new("list_present_custom_resource_kinds")
        .label("context", &cluster_context)
        .label("namespaces", namespaces.len())
        .run(
            async move {
                let access = ClusterAccess::resolve(kubeconfig_env_var.as_deref())?;
                let source_key = access.source_key().to_string();
                let store = live_store.inner().clone();
                live_store
                    .present_custom_resource_kinds(
                        source_key.clone(),
                        cluster_context.clone(),
                        namespaces.clone(),
                        move || {
                            let source_key = source_key.clone();
                            let store = store.clone();
                            async move {
                                let client = access.read_client(&cluster_context).await?;
                                let catalog_client = client.clone();
                                let catalog = store
                                    .resource_kinds(
                                        source_key,
                                        cluster_context.clone(),
                                        move || crd_resource_kinds(catalog_client),
                                    )
                                    .await?;
                                present_custom_resource_kinds_for_catalog(
                                    client,
                                    Arc::unwrap_or_clone(catalog),
                                    namespaces,
                                )
                                .await
                            }
                        },
                    )
                    .await
            },
            |rows| vec![diagnostic_field("rows", rows.len())],
        )
        .await
}

#[cfg(test)]
mod tests;
