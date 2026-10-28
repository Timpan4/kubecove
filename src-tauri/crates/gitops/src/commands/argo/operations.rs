mod payload;
mod resolve;
mod validation;

#[cfg(feature = "bench-support")]
pub use validation::benchmark_verify_kubernetes_application_identity;

use payload::{application_path, connected_payload, kubernetes_sync_payload};
use resolve::{request_connection, required_cluster_context, resolve_request};
use validation::{
    ensure_kubernetes_transport_action, fallback_allowed, valid, validate_resource_action,
    verify_application_identity, verify_kubernetes_application_identity,
};

use super::applications::application_api_resource;
use super::connected::{api_delete, api_get, api_post, ConnectedArgo};
use super::scope::{acquire_connection_lease, ConnectionLease};
use super::session::{consume, issue, peek, OperationSession, SessionSnapshot};
use crate::commands::ClusterAccess;
use crate::models::AppErrorKind;
#[cfg(test)]
use crate::models::ArgoApplicationRef;
use crate::models::{
    AppError, ArgoOperationConfirmation, ArgoOperationPreflight, ArgoOperationRequest,
    ArgoOperationResult,
};
use kube::{
    api::{Api, ApiResource, Patch, PatchParams},
    core::DynamicObject,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn preflight_argo_operation(
    request: ArgoOperationRequest,
    store: crate::runtime::State<'_, super::ArgoConnectionStore>,
) -> Result<ArgoOperationPreflight, AppError> {
    valid(&request)?;
    let access = ClusterAccess::resolve(request.kubeconfig_env_var.as_deref())?;
    if request.transport == "kubernetes" {
        ensure_kubernetes_transport_action(&request.action)?;
        fallback_allowed(&access, &request).await?;
    }
    let (resolved_request, connection_generation, connection_instance_id) =
        resolve_request(&store, &access, &request).await?;
    if resolved_request.transport == "connected" && resolved_request.action == "resourceAction" {
        let connection = request_connection(
            &store,
            &access,
            &resolved_request,
            resolved_request.application.workspace_id.as_deref(),
        )?;
        validate_resource_action(&connection, &resolved_request).await?;
    }
    let (session_id, session) = issue(
        &store.sessions,
        resolved_request,
        connection_generation,
        connection_instance_id,
    )?;
    Ok(ArgoOperationPreflight {
        allowed: true,
        transport: session.transport,
        action: session.request.action.clone(),
        reason: None,
        session_id: Some(session_id),
        expires_at: Some(session.expires_at),
        reviewed_request: Some(session.request),
    })
}

enum ExecutionBinding {
    Connected(Arc<ConnectedArgo>),
    Kubernetes { resource: ApiResource, uid: String },
}

async fn lease_connection(
    store: &super::ArgoConnectionStore,
    session: &OperationSession,
    connection: Arc<ConnectedArgo>,
    source_key: &str,
) -> Result<ConnectionLease, AppError> {
    let connection_id = session.connection_id.as_deref().ok_or_else(|| {
        AppError::new("Argo CD connection required", AppErrorKind::ArgoConnection)
    })?;
    let cluster_context = required_cluster_context(&session.request)?;
    let generation = session.connection_generation.as_deref().ok_or_else(|| {
        AppError::new(
            "Argo CD connection was replaced",
            AppErrorKind::ArgoOperationUnavailable,
        )
    })?;
    let instance_id = session.connection_instance_id.as_deref().ok_or_else(|| {
        AppError::new(
            "Argo CD connection was replaced",
            AppErrorKind::ArgoOperationUnavailable,
        )
    })?;
    acquire_connection_lease(
        store,
        connection_id,
        connection,
        cluster_context,
        session.workspace_id.as_deref(),
        source_key,
        generation,
        instance_id,
    )
    .await
}

fn reject_reviewed_operation(
    store: &super::session::SessionStore,
    id: &str,
    reviewed: &SessionSnapshot,
    error: AppError,
) -> AppError {
    let _ = consume(store, id, reviewed);
    error
}

async fn run_connected_operation(
    store: &super::ArgoConnectionStore,
    id: &str,
    reviewed: &SessionSnapshot,
    connection: Arc<ConnectedArgo>,
    access: &ClusterAccess,
) -> Result<ArgoOperationResult, AppError> {
    let lease = lease_connection(store, &reviewed.session, connection, access.source_key())
        .await
        .map_err(|error| reject_reviewed_operation(&store.sessions, id, reviewed, error))?;
    let request = consume(&store.sessions, id, reviewed)?.request;
    let path = match request.action.as_str() {
        "refresh" => application_path(&request.application, &[], Some("normal")),
        "hardRefresh" => application_path(&request.application, &[], Some("hard")),
        "sync" | "retry" => application_path(&request.application, &["sync"], None),
        "rollback" => application_path(&request.application, &["rollback"], None),
        "terminate" => application_path(&request.application, &["operation"], None),
        "resourceAction" => {
            application_path(&request.application, &["resource", "actions", "v2"], None)
        }
        _ => {
            return Err(AppError::new(
                "operation is not allowlisted",
                AppErrorKind::ArgoOperationUnavailable,
            ))
        }
    };
    let value = if request.action == "refresh" || request.action == "hardRefresh" {
        api_get(&lease.connection, &path).await?
    } else if request.action == "terminate" {
        api_delete(&lease.connection, &path).await?
    } else {
        api_post(&lease.connection, &path, connected_payload(&request)).await?
    };
    accepted("connected", value)
}

async fn run_argo_operation_for_store(
    store: &super::ArgoConnectionStore,
    confirmation: ArgoOperationConfirmation,
) -> Result<ArgoOperationResult, AppError> {
    if confirmation.confirmation != confirmation.session_id {
        return Err(AppError::new(
            "operation confirmation does not match reviewed session",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    let reviewed = peek(&store.sessions, &confirmation.session_id, None)?;
    valid(&reviewed.session.request)?;
    let (access, binding) = async {
        let access =
            ClusterAccess::resolve(reviewed.session.request.kubeconfig_env_var.as_deref())?;
        let binding = revalidate_session(store, &reviewed.session, &access).await?;
        Ok((access, binding))
    }
    .await
    .map_err(|error| {
        reject_reviewed_operation(&store.sessions, &confirmation.session_id, &reviewed, error)
    })?;
    match binding {
        ExecutionBinding::Kubernetes { resource, uid } => {
            let request = consume(&store.sessions, &confirmation.session_id, &reviewed)?.request;
            kubernetes_operation(&access, request, resource, &uid).await
        }
        ExecutionBinding::Connected(connection) => {
            run_connected_operation(
                store,
                &confirmation.session_id,
                &reviewed,
                connection,
                &access,
            )
            .await
        }
    }
}

pub async fn run_argo_operation(
    confirmation: ArgoOperationConfirmation,
    store: crate::runtime::State<'_, super::ArgoConnectionStore>,
) -> Result<ArgoOperationResult, AppError> {
    run_argo_operation_for_store(&store, confirmation).await
}

async fn revalidate_session(
    store: &super::ArgoConnectionStore,
    session: &OperationSession,
    access: &ClusterAccess,
) -> Result<ExecutionBinding, AppError> {
    let request = &session.request;
    if request.transport == "connected" {
        let connection =
            request_connection(store, access, request, session.workspace_id.as_deref())?;
        if session.connection_id != request.connection_id
            || session.connection_generation.as_deref() != Some(connection.generation.as_str())
            || session.connection_instance_id.as_deref() != Some(connection.instance_id.as_str())
        {
            return Err(AppError::new(
                "Argo CD connection was replaced",
                AppErrorKind::ArgoOperationUnavailable,
            ));
        }
        let current = api_get(
            &connection,
            &application_path(&session.application, &[], None),
        )
        .await?;
        verify_application_identity(&current, &session.application)?;
        if request.action == "resourceAction" {
            validate_resource_action(&connection, request).await?;
        }
        return Ok(ExecutionBinding::Connected(connection));
    }
    fallback_allowed(access, request).await?;
    let context = request.cluster_context.as_deref().expect("validated");
    let client = access.preview_client(context).await?;
    let resource = application_api_resource(&client).await?;
    let current = Api::<DynamicObject>::namespaced_with(
        client.clone(),
        request.application.namespace.as_deref().expect("validated"),
        &resource,
    )
    .get(&request.application.name)
    .await?;
    verify_kubernetes_application_identity(&current, &session.application)?;
    let uid = current.metadata.uid.ok_or_else(|| {
        AppError::new(
            "Application UID unavailable during operation revalidation",
            AppErrorKind::ArgoOperationUnavailable,
        )
    })?;
    Ok(ExecutionBinding::Kubernetes { resource, uid })
}

fn accepted(transport: &str, operation: Value) -> Result<ArgoOperationResult, AppError> {
    Ok(ArgoOperationResult {
        accepted: true,
        transport: transport.into(),
        message: "Argo CD accepted operation".into(),
        operation: Some(operation),
    })
}

async fn kubernetes_operation(
    access: &ClusterAccess,
    request: ArgoOperationRequest,
    resource: ApiResource,
    expected_uid: &str,
) -> Result<ArgoOperationResult, AppError> {
    ensure_kubernetes_transport_action(&request.action)?;
    let client = access
        .confirmed_operation_client(request.cluster_context.as_deref().expect("validated"))
        .await?;
    let api = Api::<DynamicObject>::namespaced_with(
        client,
        request.application.namespace.as_deref().expect("validated"),
        &resource,
    );
    let current = api.get(&request.application.name).await?;
    if current.metadata.uid.as_deref() != Some(expected_uid) {
        return Err(AppError::new(
            "Application identity changed before Kubernetes operation",
            AppErrorKind::ArgoOperationUnavailable,
        ));
    }
    verify_kubernetes_application_identity(&current, &request.application)?;
    let mut metadata = json!({
        "resourceVersion": request.resource_version.clone().expect("validated")
    });
    metadata["uid"] = Value::String(expected_uid.into());
    let patch = match request.action.as_str() {
        "refresh" | "hardRefresh" => {
            metadata["annotations"] = json!({"argocd.argoproj.io/refresh":if request.action == "hardRefresh" { "hard" } else { "normal" }});
            json!({"metadata":metadata})
        }
        "sync" | "retry" => {
            json!({"metadata":metadata,"operation":{"sync":kubernetes_sync_payload(&request)}})
        }
        _ => {
            return Err(AppError::new(
                "operation unavailable in Kubernetes fallback",
                AppErrorKind::ArgoOperationUnavailable,
            ))
        }
    };
    api.patch(
        &request.application.name,
        &PatchParams::default(),
        &Patch::Merge(&patch),
    )
    .await?;
    Ok(ArgoOperationResult {
        accepted: true,
        transport: "kubernetes".into(),
        message: "Kubernetes-authorized fallback accepted; Argo RBAC not evaluated".into(),
        operation: None,
    })
}

#[cfg(test)]
#[path = "operations_execution_tests.rs"]
mod execution_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod kubernetes_transport_gate_tests {
    use super::validation::{ensure_kubernetes_transport_action, KUBERNETES_TRANSPORT_ACTIONS};
    use crate::models::AppErrorKind;

    #[test]
    fn kubernetes_transport_allows_only_fallback_actions() {
        for action in KUBERNETES_TRANSPORT_ACTIONS {
            assert!(ensure_kubernetes_transport_action(action).is_ok());
        }
        for action in ["rollback", "terminate", "resourceAction", ""] {
            let error = ensure_kubernetes_transport_action(action).unwrap_err();
            assert_eq!(error.kind, AppErrorKind::ArgoOperationUnavailable);
        }
    }
}
