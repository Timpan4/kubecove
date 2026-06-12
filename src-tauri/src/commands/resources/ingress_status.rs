use crate::models::ResourceSummary;
use k8s_openapi::api::networking::v1::IngressStatus;

pub(super) fn apply_ingress_status(summary: &mut ResourceSummary, status: Option<&IngressStatus>) {
    let Some(status) = status else {
        return;
    };

    if has_load_balancer_address(status) {
        summary.status = Some("Ready".to_string());
        summary.ready = Some("true".to_string());
    } else {
        summary.status = Some("Pending".to_string());
        summary.ready = Some("false".to_string());
    }
}

fn has_load_balancer_address(status: &IngressStatus) -> bool {
    status
        .load_balancer
        .as_ref()
        .and_then(|load_balancer| load_balancer.ingress.as_ref())
        .is_some_and(|ingress| !ingress.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ResourceHealth;
    use k8s_openapi::api::networking::v1::{IngressLoadBalancerIngress, IngressLoadBalancerStatus};

    fn summary() -> ResourceSummary {
        ResourceSummary {
            kind: "Ingress".to_string(),
            cluster: "kind-dev".to_string(),
            name: "api".to_string(),
            namespace: Some("default".to_string()),
            age: "1m".to_string(),
            api_version: Some("networking.k8s.io/v1".to_string()),
            group: Some("networking.k8s.io".to_string()),
            version: Some("v1".to_string()),
            plural: Some("ingresses".to_string()),
            namespaced: Some(true),
            dynamic: None,
            health: ResourceHealth::Unknown,
            created_at: None,
            status: None,
            ready: None,
            restarts: None,
            owner_ref: None,
            argo_app: None,
            helm_release: None,
            git_ops_owner: None,
        }
    }

    #[test]
    fn ingress_with_load_balancer_address_is_ready() {
        let status = IngressStatus {
            load_balancer: Some(IngressLoadBalancerStatus {
                ingress: Some(vec![IngressLoadBalancerIngress {
                    ip: Some("10.0.0.1".to_string()),
                    ..Default::default()
                }]),
            }),
        };
        let mut summary = summary();

        apply_ingress_status(&mut summary, Some(&status));

        assert_eq!(summary.status.as_deref(), Some("Ready"));
        assert_eq!(summary.ready.as_deref(), Some("true"));
    }

    #[test]
    fn ingress_without_load_balancer_address_is_pending() {
        let status = IngressStatus {
            load_balancer: Some(IngressLoadBalancerStatus {
                ingress: Some(Vec::new()),
            }),
        };
        let mut summary = summary();

        apply_ingress_status(&mut summary, Some(&status));

        assert_eq!(summary.status.as_deref(), Some("Pending"));
        assert_eq!(summary.ready.as_deref(), Some("false"));
    }
}
