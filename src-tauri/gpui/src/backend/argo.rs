//! Connected Argo CD uses the same commands and credential store as the web client.
use super::*;
use kubecove_lib::models::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoConnect {
    pub profile: ArgoConnectionProfile,
    pub token: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoInspection {
    pub application: ArgoApplicationRef,
    pub connection_id: Option<String>,
}

pub(super) fn connect(backend: &NativeBackend, request: ArgoConnect) -> Read<ArgoConnectionStatus> {
    let store = backend.argo_connections.clone();
    backend.read(async move {
        let p = request.profile;
        commands::connect_argo_server(
            p.id,
            p.url,
            Some(p.endpoint),
            request.token,
            None,
            None,
            false,
            None,
            p.remember_credential,
            p.cluster_context,
            None,
            p.workspace_id,
            None,
            State::new(&*store),
        )
        .await
    })
}

pub(super) fn disconnect(backend: &NativeBackend, id: String) -> Read<()> {
    let store = backend.argo_connections.clone();
    backend.read(async move { commands::disconnect_argo_server(id, State::new(&*store)).await })
}

pub(super) fn inspect(
    backend: &NativeBackend,
    request: ArgoInspection,
) -> Read<ArgoApplicationInspector> {
    let store = backend.argo_connections.clone();
    let cancellations = backend.cancellations.clone();
    backend.read(async move {
        commands::get_argo_application_inspector(
            request.application.context.clone().unwrap_or_default(),
            None,
            request.connection_id.clone(),
            if request.connection_id.is_some() {
                "connected"
            } else {
                "kubernetes"
            }
            .into(),
            request.application,
            None,
            None,
            State::new(&*store),
            State::new(&cancellations),
        )
        .await
    })
}

pub(super) fn preflight(
    backend: &NativeBackend,
    request: ArgoOperationRequest,
) -> Read<ArgoOperationPreflight> {
    let store = backend.argo_connections.clone();
    backend
        .read(async move { commands::preflight_argo_operation(request, State::new(&*store)).await })
}

pub(super) fn run(
    backend: &NativeBackend,
    confirmation: ArgoOperationConfirmation,
) -> Read<ArgoOperationResult> {
    let store = backend.argo_connections.clone();
    backend
        .read(async move { commands::run_argo_operation(confirmation, State::new(&*store)).await })
}
