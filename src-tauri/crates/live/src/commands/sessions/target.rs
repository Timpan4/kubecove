use crate::commands::ClusterAccess;
use crate::models::AppErrorKind;
use crate::models::{AppError, PortForwardRequest};

use super::service;

const MIN_USER_LOCAL_PORT: u16 = 1024;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum PortForwardTargetKind {
    Pod,
    Service,
}

impl PortForwardTargetKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Pod => "Pod",
            Self::Service => "Service",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidatedPortForwardRequest {
    pub(super) cluster_context: String,
    pub(super) kubeconfig_env_var: Option<String>,
    pub(super) kubeconfig_source_key: Option<String>,
    pub(super) kubeconfig_source_label: Option<String>,
    pub(super) namespace: String,
    pub(super) target_kind: PortForwardTargetKind,
    pub(super) target_name: String,
    pub(super) remote_port: u16,
    pub(super) local_port: Option<u16>,
}

impl ValidatedPortForwardRequest {
    pub fn local_port(&self) -> Option<u16> {
        self.local_port
    }

    /// The target as `kubecove-server` resolved it, under the server's
    /// kubeconfig source.
    pub fn resolved_by_server(
        self,
        pod_name: String,
        pod_port: u16,
        kubeconfig_source_key: Option<String>,
        kubeconfig_source_label: Option<String>,
    ) -> PortForwardTarget {
        PortForwardTarget {
            cluster_context: self.cluster_context,
            kubeconfig_env_var: self.kubeconfig_env_var,
            kubeconfig_source_key,
            kubeconfig_source_label,
            namespace: self.namespace,
            target_kind: self.target_kind,
            target_name: self.target_name,
            pod_name,
            remote_port: self.remote_port,
            pod_port,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PortForwardTarget {
    pub(super) cluster_context: String,
    pub(super) kubeconfig_env_var: Option<String>,
    pub(super) kubeconfig_source_key: Option<String>,
    pub(super) kubeconfig_source_label: Option<String>,
    pub(super) namespace: String,
    pub(super) target_kind: PortForwardTargetKind,
    pub(super) target_name: String,
    pub(super) pod_name: String,
    pub(super) remote_port: u16,
    pub(super) pod_port: u16,
}

pub(super) fn validate_port(value: i64, field: &str) -> Result<u16, AppError> {
    if !(1..=i64::from(u16::MAX)).contains(&value) {
        return Err(AppError::new(
            format!("{field} must be between 1 and 65535"),
            AppErrorKind::Validation,
        ));
    }
    Ok(value as u16)
}

fn target_text(value: Option<&String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn validate_target_kind(request: &PortForwardRequest) -> Result<PortForwardTargetKind, AppError> {
    let target_kind =
        target_text(request.target_kind.as_ref()).unwrap_or_else(|| "Pod".to_string());
    match target_kind.as_str() {
        "Pod" => Ok(PortForwardTargetKind::Pod),
        "Service" => Ok(PortForwardTargetKind::Service),
        _ => Err(AppError::new(
            "port-forward target kind must be Pod or Service",
            AppErrorKind::Validation,
        )),
    }
}

/// Checks a request's target and ports without reading kubeconfig sources.
/// The desktop remote client checks a tunnelled forward this way; the server
/// then validates it again against the server's own sources.
pub fn validate_target(
    request: &PortForwardRequest,
) -> Result<ValidatedPortForwardRequest, AppError> {
    if request.cluster_context.trim().is_empty() || request.namespace.trim().is_empty() {
        return Err(AppError::new(
            "port-forward target is required",
            AppErrorKind::Validation,
        ));
    }
    let target_kind = validate_target_kind(request)?;
    let target_name = target_text(request.target_name.as_ref())
        .or_else(|| target_text(request.pod_name.as_ref()))
        .ok_or_else(|| {
            AppError::new("port-forward target is required", AppErrorKind::Validation)
        })?;

    crate::commands::helpers::validate_namespace(Some(request.namespace.trim()))?;
    crate::commands::helpers::validate_path_segment(&target_name, "port-forward target")?;
    let remote_port = validate_port(request.remote_port, "remote_port")?;
    // Port 0 picks a free port internally; a user-provided port must be unprivileged (ADR 0003).
    let local_port = request
        .local_port
        .map(|port| validate_port(port, "local_port"))
        .transpose()?;
    if matches!(local_port, Some(port) if port < MIN_USER_LOCAL_PORT) {
        return Err(AppError::new(
            "local_port must be 1024 or higher",
            AppErrorKind::Validation,
        ));
    }

    Ok(ValidatedPortForwardRequest {
        cluster_context: request.cluster_context.trim().to_string(),
        kubeconfig_env_var: request.kubeconfig_env_var.clone(),
        kubeconfig_source_key: None,
        kubeconfig_source_label: None,
        namespace: request.namespace.trim().to_string(),
        target_kind,
        target_name,
        remote_port,
        local_port,
    })
}

/// Validates the target and resolves the cluster access the session keeps.
pub(super) fn validate_request(
    request: &PortForwardRequest,
) -> Result<(ValidatedPortForwardRequest, ClusterAccess), AppError> {
    let mut validated = validate_target(request)?;
    let access = ClusterAccess::resolve(request.kubeconfig_env_var.as_deref())?;
    validated.kubeconfig_source_key = Some(access.source_key().to_string());
    validated.kubeconfig_source_label = access.source_label().map(str::to_string);
    Ok((validated, access))
}

pub(super) async fn resolve_port_forward_target(
    access: &ClusterAccess,
    request: ValidatedPortForwardRequest,
) -> Result<PortForwardTarget, AppError> {
    match request.target_kind {
        PortForwardTargetKind::Pod => Ok(PortForwardTarget {
            cluster_context: request.cluster_context,
            kubeconfig_env_var: request.kubeconfig_env_var,
            kubeconfig_source_key: request.kubeconfig_source_key,
            kubeconfig_source_label: request.kubeconfig_source_label,
            namespace: request.namespace,
            target_kind: PortForwardTargetKind::Pod,
            target_name: request.target_name.clone(),
            pod_name: request.target_name,
            remote_port: request.remote_port,
            pod_port: request.remote_port,
        }),
        PortForwardTargetKind::Service => {
            service::resolve_service_request_target(access, request).await
        }
    }
}
