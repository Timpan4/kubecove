use super::*;
use k8s_openapi::api::core::v1::ServiceSpec;

// Failure modes: Helm releases with custom names or label-only identity are missed; metrics
// or unrelated Services are offered; an insecure server is reached over HTTPS; the port that
// does not match the protocol is preselected; `server.rootpath` is dropped.

fn service(name: &str, labels: &[(&str, &str)]) -> Service {
    Service {
        metadata: kube::core::ObjectMeta {
            name: Some(name.into()),
            namespace: Some("argocd".into()),
            labels: Some(
                labels
                    .iter()
                    .map(|(key, value)| ((*key).into(), (*value).into()))
                    .collect(),
            ),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            ports: Some(vec![
                ServicePort {
                    name: Some("http".into()),
                    port: 80,
                    ..Default::default()
                },
                ServicePort {
                    name: Some("https".into()),
                    port: 443,
                    ..Default::default()
                },
            ]),
            selector: Some(BTreeMap::from([("app".into(), "argocd-server".into())])),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn discovery_matches_argo_server_services_by_name_or_label() {
    let operator = [
        ("app.kubernetes.io/part-of", "argocd"),
        ("app.kubernetes.io/component", "server"),
    ];
    for matching in [
        service("argocd-server", &[]),
        service("argo-cd-argocd-server", &[]),
        service("platform-argocd-server", &[]),
        service("gitops-api", &[("app.kubernetes.io/name", "argocd-server")]),
        service("openshift-gitops-server", &operator),
    ] {
        assert!(
            is_argo_server_service(&matching),
            "{:?}",
            matching.metadata.name
        );
    }
    for other in [
        service("argocd-server-metrics", &[]),
        service("argo-cd-argocd-server-metrics", &[]),
        service("openshift-gitops-server-metrics", &operator),
        service(
            "grafana-server",
            &[("app.kubernetes.io/component", "server")],
        ),
        service(
            "argocd-repo-server",
            &[("app.kubernetes.io/part-of", "argocd")],
        ),
    ] {
        assert!(!is_argo_server_service(&other), "{:?}", other.metadata.name);
    }
}

#[test]
fn server_params_choose_protocol_preferred_port_and_root_path() {
    let endpoints = |params: &ArgoServerParams| {
        servicetunnel_capabilities(&service("argocd-server", &[]), params)
            .into_iter()
            .filter_map(|capability| capability.endpoint)
            .map(|endpoint| match endpoint {
                ArgoServerEndpoint::ServiceTunnel {
                    service_port,
                    scheme,
                    root_path,
                    ..
                } => (service_port, scheme, root_path),
                ArgoServerEndpoint::ExternalHttps { .. } => unreachable!(),
            })
            .collect::<Vec<_>>()
    };
    let https = ArgoServerParams::from_config_map(None);
    assert_eq!(
        endpoints(&https),
        [(443, "https".into(), None), (80, "https".into(), None)]
    );
    let insecure = ArgoServerParams::from_config_map(Some(&BTreeMap::from([
        ("server.insecure".into(), " true ".into()),
        ("server.rootpath".into(), "/argo-cd".into()),
    ])));
    assert_eq!(
        endpoints(&insecure),
        [
            (80, "http".into(), Some("/argo-cd".into())),
            (443, "http".into(), Some("/argo-cd".into())),
        ]
    );
    let flag = |value: &str| {
        ArgoServerParams::from_config_map(Some(&BTreeMap::from([(
            "server.insecure".into(),
            value.into(),
        )])))
        .insecure
    };
    for value in ["1", "t", "True", "TRUE"] {
        assert!(flag(value), "{value} enables server.insecure");
    }
    for value in ["false", "0", "yes", "on"] {
        assert!(!flag(value), "{value} keeps HTTPS");
    }
}
