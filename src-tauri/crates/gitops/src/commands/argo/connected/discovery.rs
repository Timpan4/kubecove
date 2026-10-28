use crate::commands::gitops_crd::find_api_resource;
use crate::models::{
    kube_error_kind, AppError, AppErrorKind, ArgoServerCapability, ArgoServerEndpoint,
    ArgoServiceTunnelUnavailableReason,
};
use k8s_openapi::api::core::v1::{ConfigMap, Service, ServicePort};
use kube::{
    api::{Api, ListParams},
    core::DynamicObject,
    Client,
};
use std::collections::{BTreeMap, BTreeSet};

/// Names used by the official manifests and by the Helm chart's common release names.
const ARGO_SERVICE_NAMES: [&str; 2] = ["argocd-server", "argo-cd-argocd-server"];
/// Both the official manifests and the Helm chart keep server flags in this `ConfigMap`.
const ARGO_PARAMS_CONFIG_MAP: &str = "argocd-cmd-params-cm";

/// Server flags that decide how a tunnel must speak to the Argo CD API.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct ArgoServerParams {
    /// `server.insecure`: the API serves plain HTTP behind TLS-terminating ingress.
    pub(super) insecure: bool,
    /// `server.rootpath`: the API is served under this path.
    pub(super) root_path: Option<String>,
}

impl ArgoServerParams {
    pub(super) fn from_config_map(data: Option<&BTreeMap<String, String>>) -> Self {
        let value = |key: &str| {
            data.and_then(|data| data.get(key))
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
        };
        Self {
            // Argo CD parses this flag with Go's strconv.ParseBool.
            insecure: value("server.insecure")
                .is_some_and(|value| matches!(value, "1" | "t" | "T" | "true" | "TRUE" | "True")),
            root_path: value("server.rootpath").map(str::to_owned),
        }
    }

    fn scheme(&self) -> &'static str {
        if self.insecure {
            "http"
        } else {
            "https"
        }
    }
}

/// Lists Argo CD server Services the user can read and the tunnel endpoint for each port,
/// with the most likely endpoint first.
pub(super) async fn discover(client: &Client) -> Result<Vec<ArgoServerCapability>, AppError> {
    let mut params = BTreeMap::<String, ArgoServerParams>::new();
    let mut capabilities = Vec::new();
    for service in readable_services(client).await? {
        if !is_argo_server_service(&service) {
            continue;
        }
        let namespace = service.metadata.namespace.clone().unwrap_or_default();
        if !params.contains_key(&namespace) {
            params.insert(namespace.clone(), server_params(client, &namespace).await);
        }
        let argo_labeled = is_argo_labeled(&service);
        for mut capability in servicetunnel_capabilities(&service, &params[&namespace]) {
            capability.argo_labeled = argo_labeled;
            if let Some(ArgoServerEndpoint::ServiceTunnel {
                namespace,
                service_name,
                service_port,
                ..
            }) = capability.endpoint.as_ref()
            {
                match crate::commands::sessions::service::resolve_service_target(
                    client.clone(),
                    namespace,
                    service_name,
                    *service_port,
                )
                .await
                {
                    Ok(target) => capability.target_pod = Some(target.pod_name),
                    Err(error) => {
                        capability.unavailable = Some(tunnel_target_unavailable(&error));
                        capability.unavailable_reason = Some(error.message);
                    }
                }
            }
            capabilities.push(capability);
        }
    }
    // Stable sort keeps each Service's preferred port first.
    capabilities.sort_by_key(|capability| {
        (
            capability.unavailable_reason.is_some(),
            !capability.argo_labeled,
        )
    });
    Ok(capabilities)
}

/// Services across the cluster, or, when listing them cluster-wide is forbidden, in the
/// namespaces that hold Argo CD Applications, where the server normally runs.
async fn readable_services(client: &Client) -> Result<Vec<Service>, AppError> {
    let error = match Api::<Service>::all(client.clone())
        .list(&ListParams::default())
        .await
    {
        Ok(list) => return Ok(list.items),
        Err(error) if kube_error_kind(&error) == AppErrorKind::Forbidden => AppError::from(error),
        Err(error) => return Err(error.into()),
    };
    let Ok(namespaces) = application_namespaces(client).await else {
        return Err(error);
    };
    let mut services = Vec::new();
    let mut readable = false;
    for namespace in namespaces {
        if let Ok(list) = Api::<Service>::namespaced(client.clone(), &namespace)
            .list(&ListParams::default())
            .await
        {
            readable = true;
            services.extend(list.items);
        }
    }
    if readable {
        Ok(services)
    } else {
        Err(error)
    }
}

async fn application_namespaces(client: &Client) -> Result<BTreeSet<String>, AppError> {
    let resource = find_api_resource(client, "argoproj.io", "Application")
        .await?
        .ok_or_else(|| AppError::new("Argo CD is not installed", AppErrorKind::NotFound))?;
    let applications = Api::<DynamicObject>::all_with(client.clone(), &resource)
        .list_metadata(&ListParams::default())
        .await?;
    Ok(applications
        .items
        .into_iter()
        .filter_map(|application| application.metadata.namespace)
        .collect())
}

/// Unreadable flags fall back to the Argo CD defaults: HTTPS at the root path.
async fn server_params(client: &Client, namespace: &str) -> ArgoServerParams {
    let config_map = Api::<ConfigMap>::namespaced(client.clone(), namespace)
        .get_opt(ARGO_PARAMS_CONFIG_MAP)
        .await
        .ok()
        .flatten();
    ArgoServerParams::from_config_map(config_map.as_ref().and_then(|map| map.data.as_ref()))
}

/// Service names and labels are hints, not identity; the user confirms the target before
/// any credential is sent (ADR 0017).
pub(super) fn is_argo_server_service(service: &Service) -> bool {
    let Some(name) = service.metadata.name.as_deref() else {
        return false;
    };
    let label = |key: &str| {
        service
            .metadata
            .labels
            .as_ref()
            .and_then(|labels| labels.get(key))
            .map(String::as_str)
    };
    ARGO_SERVICE_NAMES.contains(&name)
        // Helm releases with other names: `<release>-argocd-server`.
        || name.ends_with("-argocd-server")
        // Official manifests and the Helm chart label the server Service this way.
        || label("app.kubernetes.io/name") == Some("argocd-server")
        // Argo CD Operator instances: `<instance>-server`.
        || (label("app.kubernetes.io/part-of") == Some("argocd")
            && label("app.kubernetes.io/component") == Some("server")
            && !name.ends_with("-metrics"))
}

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

/// The port that matches the server's protocol sorts first.
fn port_preference(port: &ServicePort, scheme: &str) -> u8 {
    let default_port = if scheme == "https" { 443 } else { 80 };
    let named =
        port.name.as_deref() == Some(scheme) || port.app_protocol.as_deref() == Some(scheme);
    match (named, port.port == default_port) {
        (true, _) => 0,
        (false, true) => 1,
        (false, false) => 2,
    }
}

pub(super) fn servicetunnel_capabilities(
    service: &Service,
    params: &ArgoServerParams,
) -> Vec<ArgoServerCapability> {
    let Some(name) = service.metadata.name.clone() else {
        return Vec::new();
    };
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
    let scheme = params.scheme();
    let mut tcp_ports: Vec<_> = spec
        .ports
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|port| port.protocol.as_deref().unwrap_or("TCP") == "TCP")
        .filter(|port| u16::try_from(port.port).is_ok_and(|port| port != 0))
        .collect();
    tcp_ports.sort_by_key(|port| port_preference(port, scheme));
    let ports: Vec<u16> = tcp_ports
        .into_iter()
        .filter_map(|port| u16::try_from(port.port).ok())
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
                    scheme: scheme.into(),
                    root_path: params.root_path.clone(),
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
