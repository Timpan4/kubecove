use super::*;

#[test]
fn discovery_only_offers_exact_argo_server_services() {
    let service = |name: &str| Service {
        metadata: kube::core::ObjectMeta {
            name: Some(name.into()),
            ..Default::default()
        },
        spec: Some(k8s_openapi::api::core::v1::ServiceSpec {
            ports: Some(vec![k8s_openapi::api::core::v1::ServicePort {
                port: 443,
                ..Default::default()
            }]),
            selector: Some(std::collections::BTreeMap::from([(
                "app".into(),
                "argocd-server".into(),
            )])),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(
        servicetunnel_capabilities(&service("argocd-server")).len(),
        1
    );
    assert!(servicetunnel_capabilities(&service("argocd-server-metrics")).is_empty());
    assert!(servicetunnel_capabilities(&service("argo-cd-argocd-server-metrics")).is_empty());
}
