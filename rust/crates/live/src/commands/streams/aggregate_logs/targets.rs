use super::{LogSourceKey, MAX_AGGREGATED_LOG_SOURCES};
use crate::models::{AggregatedLogStreamRequest, AppError, AppErrorKind};
use k8s_openapi::{
    api::{
        apps::v1::Deployment,
        core::v1::{Pod, Service},
    },
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::{
    Client,
    api::{Api, WatchEvent},
};
use std::collections::BTreeMap;

pub(super) async fn resolve_target_selector(
    client: Client,
    request: &AggregatedLogStreamRequest,
) -> Result<String, AppError> {
    match request.target_kind.as_str() {
        "Deployment" => {
            let deployments: Api<Deployment> = Api::namespaced(client, &request.namespace);
            let deployment = deployments
                .get(&request.target_name)
                .await
                .map_err(AppError::from)?;
            selector_from_label_selector(&deployment.spec.selector)
        }
        "Service" => {
            let services: Api<Service> = Api::namespaced(client, &request.namespace);
            let service = services
                .get(&request.target_name)
                .await
                .map_err(AppError::from)?;
            service_label_selector(&service)
        }
        _ => Err(AppError::new(
            "aggregated logs support Deployment and selector-backed Service targets",
            AppErrorKind::Validation,
        )),
    }
}

fn selector_from_label_selector(selector: &LabelSelector) -> Result<String, AppError> {
    let mut parts = Vec::new();
    if let Some(labels) = &selector.match_labels {
        parts.extend(labels.iter().map(|(key, value)| format!("{key}={value}")));
    }
    if let Some(expressions) = &selector.match_expressions {
        for expression in expressions {
            let values = expression.values.clone().unwrap_or_default();
            match expression.operator.as_str() {
                "In" => {
                    if values.is_empty() {
                        return Err(AppError::new(
                            "Deployment selector In expression needs values",
                            AppErrorKind::Validation,
                        ));
                    }
                    parts.push(format!("{} in ({})", expression.key, values.join(",")));
                }
                "NotIn" => {
                    if values.is_empty() {
                        return Err(AppError::new(
                            "Deployment selector NotIn expression needs values",
                            AppErrorKind::Validation,
                        ));
                    }
                    parts.push(format!("{} notin ({})", expression.key, values.join(",")));
                }
                "Exists" => parts.push(expression.key.clone()),
                "DoesNotExist" => parts.push(format!("!{}", expression.key)),
                operator => {
                    return Err(AppError::new(
                        format!("unsupported Deployment selector operator: {operator}"),
                        AppErrorKind::Validation,
                    ));
                }
            }
        }
    }
    if parts.is_empty() {
        return Err(AppError::new(
            "Deployment selector is empty",
            AppErrorKind::Validation,
        ));
    }
    Ok(parts.join(","))
}

fn service_label_selector(service: &Service) -> Result<String, AppError> {
    let spec = service
        .spec
        .as_ref()
        .ok_or_else(|| AppError::new("service spec is unavailable", AppErrorKind::Logs))?;
    if matches!(spec.type_.as_deref(), Some("ExternalName")) {
        return Err(AppError::new(
            "ExternalName Services do not have pod logs",
            AppErrorKind::Validation,
        ));
    }
    let selector = spec
        .selector
        .as_ref()
        .filter(|selector| !selector.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "aggregated Service logs require a selector-backed Service",
                AppErrorKind::Validation,
            )
        })?;
    Ok(map_label_selector(selector))
}

fn map_label_selector(selector: &BTreeMap<String, String>) -> String {
    selector
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Default)]
pub(super) struct PodSources {
    containers: BTreeMap<String, Vec<String>>,
    total: usize,
}

impl PodSources {
    pub(super) fn total(&self) -> usize {
        self.total
    }

    pub(super) fn from_pods(pods: &[Pod]) -> Self {
        let mut sources = Self::default();
        for pod in pods {
            sources.update(pod);
        }
        sources
    }

    pub(super) fn apply(&mut self, event: WatchEvent<Pod>) -> bool {
        match event {
            WatchEvent::Added(pod) | WatchEvent::Modified(pod) => self.update(&pod),
            WatchEvent::Deleted(pod) => pod_name(&pod).is_some_and(|name| self.remove(name)),
            WatchEvent::Bookmark(_) | WatchEvent::Error(_) => false,
        }
    }

    fn update(&mut self, pod: &Pod) -> bool {
        let Some(name) = pod_name(pod) else {
            return false;
        };
        let Some(spec) = pod
            .spec
            .as_ref()
            .filter(|_| pod.metadata.deletion_timestamp.is_none())
        else {
            return self.remove(name);
        };
        let containers = spec
            .containers
            .iter()
            .map(|container| container.name.trim())
            .filter(|name| !name.is_empty());
        let previous = self.containers.get(name).map_or(&[][..], Vec::as_slice);
        if containers.clone().eq(previous.iter().map(String::as_str)) {
            return false;
        }
        let mut containers = containers.map(str::to_owned).collect::<Vec<_>>();
        containers.sort_unstable();
        if containers.as_slice() == previous {
            return false;
        }
        self.total = self.total - previous.len() + containers.len();
        if containers.is_empty() {
            self.containers.remove(name);
        } else {
            self.containers.insert(name.to_owned(), containers);
        }
        true
    }

    fn remove(&mut self, name: &str) -> bool {
        if let Some(previous) = self.containers.remove(name) {
            self.total -= previous.len();
            true
        } else {
            false
        }
    }

    pub(super) fn selected(&self) -> Vec<LogSourceKey> {
        self.containers
            .iter()
            .flat_map(|(pod, containers)| containers.iter().map(move |container| (pod, container)))
            .take(MAX_AGGREGATED_LOG_SOURCES)
            .map(|(pod, container)| LogSourceKey {
                pod_name: pod.clone(),
                container: container.clone(),
            })
            .collect()
    }
}

fn pod_name(pod: &Pod) -> Option<&str> {
    pod.metadata
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

#[cfg(feature = "bench-support")]
pub fn benchmark_log_sources_from_pods(pods: &[Pod]) -> (Vec<LogSourceKey>, usize) {
    let sources = PodSources::from_pods(pods);
    (sources.selected(), sources.total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::{
        api::core::v1::ServiceSpec, apimachinery::pkg::apis::meta::v1::LabelSelectorRequirement,
    };

    #[test]
    fn deployment_selector_supports_match_labels_and_expressions() {
        let selector = LabelSelector {
            match_labels: Some(BTreeMap::from([(
                "app.kubernetes.io/name".to_string(),
                "api".to_string(),
            )])),
            match_expressions: Some(vec![
                LabelSelectorRequirement {
                    key: "tier".to_string(),
                    operator: "In".to_string(),
                    values: Some(vec!["web".to_string(), "worker".to_string()]),
                },
                LabelSelectorRequirement {
                    key: "canary".to_string(),
                    operator: "DoesNotExist".to_string(),
                    values: None,
                },
            ]),
        };

        assert_eq!(
            selector_from_label_selector(&selector).expect("selector"),
            "app.kubernetes.io/name=api,tier in (web,worker),!canary",
        );
    }

    #[test]
    fn deployment_selector_rejects_empty_or_invalid_expressions() {
        assert_eq!(
            selector_from_label_selector(&LabelSelector::default())
                .expect_err("empty selector")
                .message,
            "Deployment selector is empty",
        );
        let selector = LabelSelector {
            match_labels: None,
            match_expressions: Some(vec![LabelSelectorRequirement {
                key: "tier".to_string(),
                operator: "In".to_string(),
                values: None,
            }]),
        };
        assert_eq!(
            selector_from_label_selector(&selector)
                .expect_err("missing values")
                .message,
            "Deployment selector In expression needs values",
        );
    }

    #[test]
    fn service_selector_requires_selector_backed_service() {
        let selector = BTreeMap::from([
            ("app".to_string(), "api".to_string()),
            ("tier".to_string(), "web".to_string()),
        ]);
        let service = Service {
            spec: Some(ServiceSpec {
                selector: Some(selector),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            service_label_selector(&service).expect("selector"),
            "app=api,tier=web",
        );

        let empty = Service {
            spec: Some(ServiceSpec {
                selector: None,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            service_label_selector(&empty)
                .expect_err("selectorless")
                .kind,
            AppErrorKind::Validation,
        );
    }
}
