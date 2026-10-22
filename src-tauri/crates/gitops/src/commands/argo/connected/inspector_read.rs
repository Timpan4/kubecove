use super::super::scope::scoped_connection;
use super::super::transport::{api_get, argo_url, redact_secret_fields};
use super::inspection::{connected_inspector, inspector_from_application};
use super::store::ArgoConnectionStore;
use crate::commands::{
    gitops_crd::{find_api_resource, get_crd_object},
    ClusterAccess,
};
use crate::models::AppErrorKind;
use crate::models::{AppError, ArgoApplicationInspector, ArgoApplicationRef};
use kube::core::DynamicObject;

pub(crate) async fn kubernetes_application(
    access: &ClusterAccess,
    cluster_context: &str,
    namespace: Option<&str>,
    name: &str,
) -> Result<DynamicObject, AppError> {
    let client = access.read_client(cluster_context).await?;
    let ar = find_api_resource(&client, "argoproj.io", "Application")
        .await?
        .ok_or_else(|| AppError::new("Application CRD not found", AppErrorKind::Cluster))?;
    get_crd_object(client, &ar, name, namespace).await
}

async fn kubernetes_inspector(
    access: &ClusterAccess,
    cluster_context: &str,
    application: &ArgoApplicationRef,
) -> Result<ArgoApplicationInspector, AppError> {
    let workspace_id = application.workspace_id.clone();
    let mut object = kubernetes_application(
        access,
        cluster_context,
        application.namespace.as_deref(),
        &application.name,
    )
    .await?;
    redact_secret_fields(&mut object.data);
    let mut inspector = inspector_from_application(&object)?;
    inspector.application.context = Some(cluster_context.to_string());
    inspector.application.workspace_id = workspace_id;
    Ok(inspector)
}

pub(crate) fn connected_application_path(
    base: &str,
    name: &str,
    namespace: Option<&str>,
    project: Option<&str>,
    managed_resources: bool,
) -> Result<String, AppError> {
    argo_url(base)?;
    let mut path = reqwest::Url::parse("https://argo.invalid/")
        .expect("constant Argo CD API base URL is valid");
    {
        let mut segments = path.path_segments_mut().map_err(|()| {
            AppError::new("invalid Argo CD API path", AppErrorKind::ArgoConnection)
        })?;
        segments.extend(["api", "v1", "applications"]).push(name);
        if managed_resources {
            segments.push("managed-resources");
        }
    }
    let mut query = path.query_pairs_mut();
    if let Some(value) = namespace {
        query.append_pair("appNamespace", value);
    }
    if let Some(value) = project {
        query.append_pair("project", value);
    }
    drop(query);
    Ok(format!(
        "{}{}",
        path.path(),
        path.query()
            .map(|value| format!("?{value}"))
            .unwrap_or_default()
    ))
}

async fn connected_inspector_read(
    store: &ArgoConnectionStore,
    cluster_context: &str,
    source_key: &str,
    connection_id: Option<&str>,
    application: ArgoApplicationRef,
) -> Result<ArgoApplicationInspector, AppError> {
    let connection = scoped_connection(
        store,
        connection_id.ok_or_else(|| {
            AppError::new("Argo CD connection required", AppErrorKind::ArgoConnection)
        })?,
        cluster_context,
        application.workspace_id.as_deref(),
        source_key,
    )?;
    let namespace = application.namespace.clone().unwrap_or_default();
    let response = api_get(
        &connection,
        &connected_application_path(
            &connection.profile.url,
            &application.name,
            Some(&namespace),
            application.project.as_deref(),
            false,
        )?,
    )
    .await?;
    let managed = api_get(
        &connection,
        &connected_application_path(
            &connection.profile.url,
            &application.name,
            Some(&namespace),
            None,
            true,
        )?,
    )
    .await?;
    Ok(connected_inspector(application, response, managed))
}

fn inspection_failure(error: &AppError) -> crate::models::ArgoInspectionFailure {
    let message = match error.kind {
        AppErrorKind::Forbidden => "access denied",
        AppErrorKind::NotFound => "Application not found",
        AppErrorKind::Cancelled => "request cancelled",
        AppErrorKind::Kubeconfig => "cluster configuration unavailable",
        AppErrorKind::Network => "network unavailable",
        AppErrorKind::ArgoConnection => "connection unavailable",
        AppErrorKind::ArgoApi => "Argo CD API request failed",
        AppErrorKind::ProviderDiscoveryUnavailable => "Application API unavailable",
        AppErrorKind::Serialization => "invalid response",
        _ => "read failed",
    };
    let kind = match error.kind {
        AppErrorKind::ArgoApi
        | AppErrorKind::ArgoConnection
        | AppErrorKind::Cancelled
        | AppErrorKind::Cluster
        | AppErrorKind::Forbidden
        | AppErrorKind::Kubeconfig
        | AppErrorKind::Network
        | AppErrorKind::NotFound
        | AppErrorKind::ProviderDiscoveryUnavailable
        | AppErrorKind::Serialization => error.kind.as_str(),
        _ => "inspection",
    };
    crate::models::ArgoInspectionFailure {
        kind: kind.into(),
        message: message.into(),
    }
}

fn with_connected_fallback(
    mut inspector: ArgoApplicationInspector,
    error: &AppError,
) -> ArgoApplicationInspector {
    inspector.connected_fallback = Some(crate::models::ArgoConnectedFallback {
        transport: "connected".into(),
        failure: inspection_failure(error),
    });
    inspector
}

fn inspection_error(connected: &AppError, kubernetes: &AppError) -> AppError {
    let connected = inspection_failure(connected);
    let kubernetes = inspection_failure(kubernetes);
    AppError::new(
        format!(
            "Connected inspection failed ({}) {}; Kubernetes inspection failed ({}) {}",
            connected.kind, connected.message, kubernetes.kind, kubernetes.message
        ),
        AppErrorKind::ArgoInspection,
    )
}

pub(super) async fn application_inspector(
    store: &ArgoConnectionStore,
    access: &ClusterAccess,
    cluster_context: String,
    connection_id: Option<String>,
    transport: String,
    application: ArgoApplicationRef,
) -> Result<ArgoApplicationInspector, AppError> {
    if transport == "kubernetes" {
        return kubernetes_inspector(access, &cluster_context, &application).await;
    }
    if transport != "connected" {
        return Err(AppError::new(
            "invalid Argo CD transport",
            AppErrorKind::ArgoConnection,
        ));
    }
    match connected_inspector_read(
        store,
        &cluster_context,
        access.source_key(),
        connection_id.as_deref(),
        application.clone(),
    )
    .await
    {
        Ok(inspector) => Ok(inspector),
        Err(connected_error) => {
            match kubernetes_inspector(access, &cluster_context, &application).await {
                Ok(inspector) => Ok(with_connected_fallback(inspector, &connected_error)),
                Err(kubernetes_error) => Err(inspection_error(&connected_error, &kubernetes_error)),
            }
        }
    }
}

#[cfg(test)]
mod tests;
