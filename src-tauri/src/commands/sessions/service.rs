use super::{
    client_for_context, validate_port, PortForwardTarget, PortForwardTargetKind,
    ValidatedPortForwardRequest,
};
use crate::models::AppError;
use k8s_openapi::{
    api::core::v1::{Pod, Service, ServicePort},
    apimachinery::pkg::util::intstr::IntOrString,
};
use kube::api::{Api, ListParams};
use std::collections::BTreeMap;

pub(super) async fn resolve_service_target(
    request: ValidatedPortForwardRequest,
) -> Result<PortForwardTarget, AppError> {
    let client =
        client_for_context(&request.cluster_context, request.kubeconfig_env_var.clone()).await?;
    let services: Api<Service> = Api::namespaced(client.clone(), &request.namespace);
    let service = services
        .get(&request.target_name)
        .await
        .map_err(|err| AppError::kube(err.to_string()))?;
    let service_spec = service
        .spec
        .ok_or_else(|| AppError::new("service spec is unavailable", "session"))?;
    if matches!(service_spec.type_.as_deref(), Some("ExternalName")) {
        return Err(AppError::new(
            "ExternalName Services cannot be port-forwarded",
            "session",
        ));
    }

    let service_port = select_service_port(
        service_spec.ports.as_deref().unwrap_or_default(),
        request.remote_port,
    )?;
    let selector = service_spec
        .selector
        .filter(|selector| !selector.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "service port-forwarding requires a selector-backed Service",
                "session",
            )
        })?;

    let pods: Api<Pod> = Api::namespaced(client, &request.namespace);
    let pod_list = pods
        .list(&ListParams::default().labels(&label_selector(&selector)))
        .await
        .map_err(|err| AppError::kube(err.to_string()))?;
    let pod = select_ready_pod(pod_list.items)
        .ok_or_else(|| AppError::new("no ready Pods matched this Service selector", "session"))?;
    let pod_name = pod_name(&pod)
        .ok_or_else(|| AppError::new("resolved Service target Pod is missing a name", "session"))?;
    let pod_port = resolve_service_target_port(&pod, service_port)?;

    Ok(PortForwardTarget {
        cluster_context: request.cluster_context,
        kubeconfig_env_var: request.kubeconfig_env_var,
        kubeconfig_source_key: request.kubeconfig_source_key,
        kubeconfig_source_label: request.kubeconfig_source_label,
        namespace: request.namespace,
        target_kind: PortForwardTargetKind::Service,
        target_name: request.target_name,
        pod_name,
        remote_port: request.remote_port,
        pod_port,
    })
}

fn label_selector(selector: &BTreeMap<String, String>) -> String {
    selector
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn select_service_port(
    ports: &[ServicePort],
    requested_port: u16,
) -> Result<&ServicePort, AppError> {
    if ports.is_empty() {
        return Err(AppError::new("service has no ports to forward", "session"));
    }
    let Some(port) = ports
        .iter()
        .find(|port| port.port == i32::from(requested_port))
    else {
        let available = ports
            .iter()
            .map(|port| port.port.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(AppError::new(
            format!("service port {requested_port} was not found; available ports: {available}"),
            "validation",
        ));
    };
    if matches!(port.protocol.as_deref(), Some(protocol) if protocol != "TCP") {
        return Err(AppError::new(
            "only TCP Service ports can be forwarded",
            "validation",
        ));
    }
    Ok(port)
}

fn select_ready_pod(mut pods: Vec<Pod>) -> Option<Pod> {
    pods.sort_by_key(pod_name);
    pods.into_iter().find(is_ready_running_pod)
}

fn is_ready_running_pod(pod: &Pod) -> bool {
    if pod.metadata.deletion_timestamp.is_some() {
        return false;
    }
    let Some(status) = &pod.status else {
        return false;
    };
    if status.phase.as_deref() != Some("Running") {
        return false;
    }
    status.conditions.as_ref().is_some_and(|conditions| {
        conditions
            .iter()
            .any(|condition| condition.type_ == "Ready" && condition.status == "True")
    })
}

fn pod_name(pod: &Pod) -> Option<String> {
    pod.metadata
        .name
        .as_ref()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

fn resolve_service_target_port(pod: &Pod, service_port: &ServicePort) -> Result<u16, AppError> {
    match service_port.target_port.as_ref() {
        Some(IntOrString::Int(port)) => validate_port(i64::from(*port), "service targetPort"),
        Some(IntOrString::String(name)) => find_named_container_port(pod, name),
        None => validate_port(i64::from(service_port.port), "service port"),
    }
}

fn find_named_container_port(pod: &Pod, port_name: &str) -> Result<u16, AppError> {
    let Some(spec) = &pod.spec else {
        return Err(AppError::new(
            "resolved Service target Pod is missing a spec",
            "session",
        ));
    };

    spec.containers
        .iter()
        .flat_map(|container| container.ports.iter().flatten())
        .find(|port| port.name.as_deref() == Some(port_name))
        .map(|port| validate_port(i64::from(port.container_port), "container port"))
        .transpose()?
        .ok_or_else(|| {
            AppError::new(
                format!("Service targetPort '{port_name}' was not found on the resolved Pod"),
                "session",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{Container, ContainerPort, PodCondition, PodSpec, PodStatus};
    use kube::core::ObjectMeta;

    fn pod(name: &str, ready: bool, ports: Vec<ContainerPort>) -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..Default::default()
            },
            spec: Some(PodSpec {
                containers: vec![Container {
                    name: "app".to_string(),
                    ports: Some(ports),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            status: Some(PodStatus {
                phase: Some("Running".to_string()),
                conditions: Some(vec![PodCondition {
                    type_: "Ready".to_string(),
                    status: if ready { "True" } else { "False" }.to_string(),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn selects_requested_tcp_service_port() {
        let ports = vec![
            ServicePort {
                port: 80,
                ..Default::default()
            },
            ServicePort {
                port: 443,
                protocol: Some("TCP".to_string()),
                ..Default::default()
            },
        ];

        assert_eq!(select_service_port(&ports, 443).expect("port").port, 443);
        assert_eq!(
            select_service_port(&ports, 8080)
                .expect_err("missing port")
                .message,
            "service port 8080 was not found; available ports: 80, 443",
        );
    }

    #[test]
    fn resolves_named_service_target_port_from_selected_pod() {
        let pod = pod(
            "api-0",
            true,
            vec![ContainerPort {
                name: Some("http".to_string()),
                container_port: 8080,
                ..Default::default()
            }],
        );
        let service_port = ServicePort {
            port: 80,
            target_port: Some(IntOrString::String("http".to_string())),
            ..Default::default()
        };

        assert_eq!(
            resolve_service_target_port(&pod, &service_port).expect("target port"),
            8080,
        );
    }

    #[test]
    fn selects_ready_running_pod_by_name() {
        let pods = vec![pod("api-b", true, vec![]), pod("api-a", true, vec![])];

        assert_eq!(
            pod_name(&select_ready_pod(pods).expect("pod")).as_deref(),
            Some("api-a"),
        );
    }

    #[test]
    fn ignores_unready_pods_for_service_resolution() {
        assert!(select_ready_pod(vec![pod("api-0", false, vec![])]).is_none());
    }
}
