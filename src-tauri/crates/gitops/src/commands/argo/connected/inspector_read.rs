use super::super::scope::scoped_connection;
use super::super::transport::{api_get, application_api_path, argo_url, is_response_too_large};
use super::inspection::{connected_inspector, kubernetes_inspector_from_object};
use super::store::ArgoConnectionStore;
use crate::commands::{
    argo::applications::application_api_resource, gitops_crd::get_crd_object, ClusterAccess,
};
use crate::models::AppErrorKind;
use crate::models::{AppError, ArgoApplicationInspector, ArgoApplicationRef, ArgoManagedResource};
use kube::core::DynamicObject;

pub(crate) async fn kubernetes_application(
    access: &ClusterAccess,
    cluster_context: &str,
    namespace: Option<&str>,
    name: &str,
) -> Result<DynamicObject, AppError> {
    let client = access.read_client(cluster_context).await?;
    let ar = application_api_resource(&client).await?;
    get_crd_object(client, &ar, name, namespace).await
}

async fn kubernetes_inspector(
    access: &ClusterAccess,
    cluster_context: &str,
    application: &ArgoApplicationRef,
) -> Result<ArgoApplicationInspector, AppError> {
    let object = kubernetes_application(
        access,
        cluster_context,
        application.namespace.as_deref(),
        &application.name,
    )
    .await?;
    kubernetes_inspector_from_object(object, cluster_context, application.workspace_id.clone())
}

pub(crate) fn connected_application_path(
    base: &str,
    name: &str,
    namespace: Option<&str>,
    project: Option<&str>,
    managed_resources: bool,
) -> Result<String, AppError> {
    argo_url(base)?;
    let segments: &[&str] = if managed_resources {
        &["managed-resources"]
    } else {
        &[]
    };
    Ok(application_api_path(
        name,
        segments,
        &[("appNamespace", namespace), ("project", project)],
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
    let managed = match api_get(
        &connection,
        &connected_application_path(
            &connection.profile.url,
            &application.name,
            Some(&namespace),
            None,
            true,
        )?,
    )
    .await
    {
        Ok(managed) => Some(managed),
        // Every manifest at once can exceed the response cap, as for Argo CD's own CRDs.
        // The Application status still lists the resources.
        Err(error) if is_response_too_large(&error) => None,
        Err(error) => return Err(error),
    };
    Ok(connected_inspector(application, response, managed))
}

/// One managed resource's states. Argo CD filters the list server-side, so selecting
/// a resource does not download every manifest of the Application.
pub(crate) fn managed_resource_path(
    base: &str,
    application: &ArgoApplicationRef,
    resource: &ArgoManagedResource,
) -> Result<String, AppError> {
    argo_url(base)?;
    Ok(application_api_path(
        &application.name,
        &["managed-resources"],
        &[
            ("appNamespace", application.namespace.as_deref()),
            ("namespace", resource.namespace.as_deref()),
            ("name", resource.name.as_deref()),
            ("version", resource.version.as_deref()),
            ("group", resource.group.as_deref()),
            ("kind", resource.kind.as_deref()),
        ],
    ))
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
