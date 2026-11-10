//! Connected Argo CD uses the same commands and credential store as the web client.
use super::*;
use kubecove_lib::models::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoConnect {
    pub profile: ArgoConnectionProfile,
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub insecure_tls: bool,
    pub custom_ca_pem: Option<Vec<u8>>,
    pub confirmed_target: Option<ArgoConfirmedTarget>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoInspection {
    pub application: ArgoApplicationRef,
    pub connection_id: Option<String>,
    pub source: Option<String>,
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
            request.username,
            request.password,
            request.insecure_tls,
            request.custom_ca_pem,
            p.remember_credential,
            p.cluster_context,
            p.kubeconfig_source_key,
            p.workspace_id,
            request.confirmed_target,
            State::new(&*store),
        )
        .await
    })
}

pub(super) fn disconnect(backend: &NativeBackend, id: String) -> Read<()> {
    let store = backend.argo_connections.clone();
    backend.read(async move { commands::disconnect_argo_server(id, State::new(&*store)).await })
}

pub(super) fn forget(backend: &NativeBackend, profile: ArgoConnectionProfile) -> Read<()> {
    backend.read(async move { commands::forget_argo_credential(profile) })
}

pub(super) fn compare(
    backend: &NativeBackend,
    request: ArgoInspection,
    resource: ArgoManagedResource,
) -> Read<ArgoResourceComparison> {
    let store = backend.argo_connections.clone();
    let cancellations = backend.cancellations.clone();
    backend.read(async move {
        commands::get_argo_resource_comparison(
            request.application.context.clone().unwrap_or_default(),
            request.source,
            request.connection_id,
            "connected".into(),
            request.application,
            resource,
            None,
            None,
            State::new(&*store),
            State::new(&cancellations),
        )
        .await
    })
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
            request.source,
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoTarget {
    pub context: String,
    pub source: Option<String>,
    pub endpoint: Option<ArgoServerEndpoint>,
}
pub(super) fn discover(backend: &NativeBackend, r: ArgoTarget) -> Read<Vec<ArgoServerCapability>> {
    let cancellations = backend.cancellations.clone();
    backend.read(async move {
        commands::discover_argo_servers(r.context, r.source, None, None, State::new(&cancellations))
            .await
    })
}
pub(super) fn preview(backend: &NativeBackend, r: ArgoTarget) -> Read<ArgoTunnelTargetPreview> {
    backend.read(async move {
        let endpoint = r.endpoint.ok_or_else(|| {
            AppError::new(
                "Select a Service tunnel",
                kubecove_lib::models::AppErrorKind::ArgoConnection,
            )
        })?;
        commands::preview_argo_tunnel_target(r.context, r.source, endpoint).await
    })
}
macro_rules! defaults {
    () => {
        fn forget_argo(&self, _: kubecove_lib::models::ArgoConnectionProfile) -> Read<()> {
            settings_integrations::unavailable("Argo CD credential deletion unavailable")
        }
        fn compare_argo(
            &self,
            _: ArgoInspection,
            _: kubecove_lib::models::ArgoManagedResource,
        ) -> Read<kubecove_lib::models::ArgoResourceComparison> {
            settings_integrations::unavailable("Argo CD resource comparison unavailable")
        }
        fn connect_argo(&self, _: ArgoConnect) -> Read<kubecove_lib::models::ArgoConnectionStatus> {
            settings_integrations::unavailable("Argo CD connection unavailable")
        }
        fn disconnect_argo(&self, _: String) -> Read<()> {
            settings_integrations::unavailable("Argo CD connection unavailable")
        }
        fn inspect_argo(&self, request: ArgoInspection) -> Read<ArgoApplicationInspector> {
            if request.connection_id.is_some() {
                return settings_integrations::unavailable("Connected inspection unavailable");
            }
            self.argo_inspector(request.application)
        }
        fn preflight_argo(
            &self,
            _: kubecove_lib::models::ArgoOperationRequest,
        ) -> Read<kubecove_lib::models::ArgoOperationPreflight> {
            settings_integrations::unavailable("Argo CD operation unavailable")
        }
        fn run_argo(
            &self,
            _: kubecove_lib::models::ArgoOperationConfirmation,
        ) -> Read<kubecove_lib::models::ArgoOperationResult> {
            settings_integrations::unavailable("Argo CD operation unavailable")
        }
        fn discover_argo(
            &self,
            _: ArgoTarget,
        ) -> Read<Vec<kubecove_lib::models::ArgoServerCapability>> {
            settings_integrations::unavailable("Argo CD discovery unavailable")
        }
        fn preview_argo(
            &self,
            _: ArgoTarget,
        ) -> Read<kubecove_lib::models::ArgoTunnelTargetPreview> {
            settings_integrations::unavailable("Argo CD tunnel preview unavailable")
        }
    };
}
pub(super) use defaults;
macro_rules! native {
    () => {
        fn forget_argo(&self, profile: kubecove_lib::models::ArgoConnectionProfile) -> Read<()> {
            argo::forget(self, profile)
        }
        fn compare_argo(
            &self,
            request: ArgoInspection,
            resource: kubecove_lib::models::ArgoManagedResource,
        ) -> Read<kubecove_lib::models::ArgoResourceComparison> {
            argo::compare(self, request, resource)
        }
        fn connect_argo(
            &self,
            request: ArgoConnect,
        ) -> Read<kubecove_lib::models::ArgoConnectionStatus> {
            argo::connect(self, request)
        }
        fn disconnect_argo(&self, id: String) -> Read<()> {
            argo::disconnect(self, id)
        }
        fn inspect_argo(&self, request: ArgoInspection) -> Read<ArgoApplicationInspector> {
            argo::inspect(self, request)
        }
        fn preflight_argo(
            &self,
            request: kubecove_lib::models::ArgoOperationRequest,
        ) -> Read<kubecove_lib::models::ArgoOperationPreflight> {
            argo::preflight(self, request)
        }
        fn run_argo(
            &self,
            confirmation: kubecove_lib::models::ArgoOperationConfirmation,
        ) -> Read<kubecove_lib::models::ArgoOperationResult> {
            argo::run(self, confirmation)
        }
        fn discover_argo(
            &self,
            request: ArgoTarget,
        ) -> Read<Vec<kubecove_lib::models::ArgoServerCapability>> {
            argo::discover(self, request)
        }
        fn preview_argo(
            &self,
            request: ArgoTarget,
        ) -> Read<kubecove_lib::models::ArgoTunnelTargetPreview> {
            argo::preview(self, request)
        }
    };
}
pub(super) use native;
