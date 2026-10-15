use crate::models::AppErrorKind;
use crate::models::{
    AppError, ArgoServerCapability, ArgoServerEndpoint, ArgoServiceTunnelUnavailableReason,
};
use k8s_openapi::api::core::v1::Service;

const ARGO_SERVICE_NAMES: [&str; 2] = ["argocd-server", "argo-cd-argocd-server"];

fn unavailable_capability(
    id: String,
    name: String,
    namespace: Option<String>,
    reason: ArgoServiceTunnelUnavailableReason,
    message: impl Into<String>,
) -> ArgoServerCapability {
    ArgoServerCapability {
        id,
        name,
        namespace,
        url: None,
        transport: "serviceTunnel".into(),
        endpoint: None,
        unavailable_reason: Some(message.into()),
        unavailable: Some(reason),
        target_pod: None,
        argo_labeled: false,
    }
}

pub(super) fn servicetunnel_capabilities(service: &Service) -> Vec<ArgoServerCapability> {
    let Some(name) = service.metadata.name.clone() else {
        return Vec::new();
    };
    if !ARGO_SERVICE_NAMES.contains(&name.as_str()) {
        return Vec::new();
    }
    let namespace = service.metadata.namespace.clone();
    let id = format!("service:{}:{name}", namespace.clone().unwrap_or_default());
    let Some(spec) = service.spec.as_ref() else {
        return vec![unavailable_capability(
            id,
            name,
            namespace,
            ArgoServiceTunnelUnavailableReason::TargetUnavailable,
            "service spec is unavailable",
        )];
    };
    if matches!(spec.type_.as_deref(), Some("ExternalName")) {
        return vec![unavailable_capability(
            id,
            name,
            namespace,
            ArgoServiceTunnelUnavailableReason::ExternalName,
            "ExternalName Services cannot be port-forwarded",
        )];
    }
    let ports: Vec<_> = spec
        .ports
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|port| port.protocol.as_deref().unwrap_or("TCP") == "TCP")
        .filter_map(|port| u16::try_from(port.port).ok())
        .filter(|port| *port != 0)
        .collect();
    if ports.is_empty() {
        return vec![unavailable_capability(
            id,
            name,
            namespace,
            ArgoServiceTunnelUnavailableReason::NoTcpPorts,
            "service has no TCP ports to forward",
        )];
    }
    let selector_missing = spec
        .selector
        .as_ref()
        .is_none_or(std::collections::BTreeMap::is_empty);
    ports
        .into_iter()
        .map(|service_port| {
            let id = format!("{id}:{service_port}");
            if selector_missing {
                return unavailable_capability(
                    id,
                    name.clone(),
                    namespace.clone(),
                    ArgoServiceTunnelUnavailableReason::SelectorRequired,
                    "service port-forwarding requires a selector-backed Service",
                );
            }
            ArgoServerCapability {
                id,
                name: name.clone(),
                namespace: namespace.clone(),
                url: None,
                transport: "serviceTunnel".into(),
                endpoint: Some(ArgoServerEndpoint::ServiceTunnel {
                    namespace: namespace.clone().unwrap_or_default(),
                    service_name: name.clone(),
                    service_port,
                    scheme: "https".into(),
                    root_path: None,
                    tls_server_name: None,
                }),
                unavailable_reason: None,
                unavailable: None,
                target_pod: None,
                argo_labeled: false,
            }
        })
        .collect()
}

pub(super) fn is_argo_labeled(service: &Service) -> bool {
    service
        .metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get("app.kubernetes.io/part-of"))
        .is_some_and(|value| value == "argocd")
}

pub(super) fn tunnel_target_unavailable(error: &AppError) -> ArgoServiceTunnelUnavailableReason {
    match error.kind {
        AppErrorKind::LiveSessionTargetUnavailable => {
            ArgoServiceTunnelUnavailableReason::NoReadyPod
        }
        _ => ArgoServiceTunnelUnavailableReason::TargetUnavailable,
    }
}

#[cfg(test)]
mod tests;
