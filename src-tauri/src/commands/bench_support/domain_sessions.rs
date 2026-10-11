use k8s_openapi::api::core::v1::{
    Container, ContainerPort, Pod, PodCondition, PodSpec, PodStatus, ServicePort,
};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::ObjectMeta;

use crate::{
    commands::{
        pod_exec::benchmark_pod_exec_session_summary,
        sessions::{
            benchmark_port_forward_session_summary, service::benchmark_resolve_ready_service_target,
        },
    },
    models::{AppError, PodExecSessionSummary, PortForwardSessionSummary},
};

pub fn target_inputs() -> (Vec<Pod>, ServicePort, ServicePort) {
    let pods = vec![
        pod(
            "api-b",
            true,
            vec![ContainerPort {
                name: Some("http".to_string()),
                container_port: 8080,
                ..Default::default()
            }],
        ),
        deleted_pod(),
        pod("api-a", true, Vec::new()),
        pod("api-unready", false, vec![]),
        Pod {
            metadata: ObjectMeta::default(),
            spec: None,
            status: None,
        },
    ];
    let named_port = ServicePort {
        port: 80,
        target_port: Some(IntOrString::String("http".to_string())),
        ..Default::default()
    };
    let numeric_port = ServicePort {
        port: 80,
        target_port: Some(IntOrString::Int(9090)),
        ..Default::default()
    };
    (pods, named_port, numeric_port)
}

pub fn resolve_service_target(
    pods: Vec<Pod>,
    service_port: ServicePort,
) -> Result<(String, u16), AppError> {
    benchmark_resolve_ready_service_target(pods, service_port)
}

pub fn port_forward_session_summary(pod_name: String, pod_port: u16) -> PortForwardSessionSummary {
    benchmark_port_forward_session_summary(pod_name, pod_port)
}

pub fn pod_exec_session_summary() -> PodExecSessionSummary {
    benchmark_pod_exec_session_summary()
}

pub fn encode<T: serde::Serialize>(value: &T) -> String {
    super::encode_complete_response(value)
}

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

fn deleted_pod() -> Pod {
    Pod {
        metadata: ObjectMeta {
            name: Some("api-0".to_string()),
            deletion_timestamp: Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(
                k8s_openapi::jiff::Timestamp::from_second(0).expect("fixture timestamp"),
            )),
            ..Default::default()
        },
        spec: Some(PodSpec {
            containers: vec![Container {
                name: "app".to_string(),
                ports: Some(vec![ContainerPort {
                    name: Some("http".to_string()),
                    container_port: 9999,
                    ..Default::default()
                }]),
                ..Default::default()
            }],
            ..Default::default()
        }),
        status: Some(PodStatus {
            phase: Some("Running".to_string()),
            conditions: Some(vec![PodCondition {
                type_: "Ready".to_string(),
                status: "True".to_string(),
                ..Default::default()
            }]),
            ..Default::default()
        }),
    }
}
