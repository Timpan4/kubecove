use super::builtin_summary::{BuiltinSummary, KindStatus};
use crate::models::ResourceHealth;
use k8s_openapi::api::networking::v1::{Ingress, IngressStatus};
use serde_json::Value;

impl BuiltinSummary for Ingress {
    fn kind_status(&self) -> Option<KindStatus> {
        let Some(status) = &self.status else {
            return Some(KindStatus::default());
        };
        let (status, ready, health) = if has_load_balancer_address(status) {
            ("Ready", "true", ResourceHealth::Healthy)
        } else {
            ("Pending", "false", ResourceHealth::Attention)
        };
        Some(KindStatus {
            status: Some(status.to_string()),
            ready: Some(ready.to_string()),
            restarts: None,
            health,
        })
    }

    fn raw_status(&self) -> Option<serde_json::Result<Value>> {
        self.status.as_ref().map(serde_json::to_value)
    }
}

fn has_load_balancer_address(status: &IngressStatus) -> bool {
    status
        .load_balancer
        .as_ref()
        .and_then(|load_balancer| load_balancer.ingress.as_ref())
        .is_some_and(|ingress| {
            ingress.iter().any(|entry| {
                [&entry.ip, &entry.hostname]
                    .into_iter()
                    .flatten()
                    .any(|address| !address.trim().is_empty())
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::networking::v1::{IngressLoadBalancerIngress, IngressLoadBalancerStatus};

    fn status_and_ready(entries: Option<Vec<IngressLoadBalancerIngress>>) -> (String, String) {
        let ingress = Ingress {
            status: Some(IngressStatus {
                load_balancer: Some(IngressLoadBalancerStatus { ingress: entries }),
            }),
            ..Ingress::default()
        };
        let status = ingress.kind_status().expect("Ingress status");
        (status.status.unwrap(), status.ready.unwrap())
    }

    #[test]
    fn ingress_with_load_balancer_address_is_ready() {
        let entry = IngressLoadBalancerIngress {
            ip: Some("10.0.0.1".to_string()),
            ..Default::default()
        };

        assert_eq!(
            status_and_ready(Some(vec![entry])),
            ("Ready".to_string(), "true".to_string())
        );
    }

    #[test]
    fn ingress_without_load_balancer_address_is_pending() {
        assert_eq!(
            status_and_ready(Some(Vec::new())),
            ("Pending".to_string(), "false".to_string())
        );
    }

    #[test]
    fn ingress_with_empty_load_balancer_entry_is_pending() {
        assert_eq!(
            status_and_ready(Some(vec![IngressLoadBalancerIngress::default()])),
            ("Pending".to_string(), "false".to_string())
        );
    }

    #[test]
    fn ingress_with_blank_load_balancer_address_is_pending() {
        let entry = IngressLoadBalancerIngress {
            ip: Some(String::new()),
            hostname: Some("  ".to_string()),
            ..Default::default()
        };

        assert_eq!(
            status_and_ready(Some(vec![entry])),
            ("Pending".to_string(), "false".to_string())
        );
    }
}
