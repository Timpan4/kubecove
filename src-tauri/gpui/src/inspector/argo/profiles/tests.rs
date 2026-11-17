//! The same failure modes as the web's `argo-connection-policy.test.ts`. The parity fixture
//! takes its profile ids from the web, so a scenario also fails when the two rules drift.
use super::{endpoint_identity, saved_profile_id};
use kubecove_lib::models::{ArgoConnectionProfile, ArgoServerEndpoint};

fn external(url: &str) -> ArgoServerEndpoint {
    ArgoServerEndpoint::ExternalHttps { url: url.into() }
}
fn tunnel(namespace: &str) -> ArgoServerEndpoint {
    ArgoServerEndpoint::ServiceTunnel {
        namespace: namespace.into(),
        service_name: "argocd-server".into(),
        service_port: 443,
        scheme: "https".into(),
        root_path: None,
        tls_server_name: None,
    }
}
fn profile(id: &str, endpoint: ArgoServerEndpoint) -> ArgoConnectionProfile {
    ArgoConnectionProfile {
        id: id.into(),
        url: String::new(),
        endpoint,
        cluster_context: None,
        workspace_id: None,
        kubeconfig_source_key: None,
        transport: "connected".into(),
        remember_credential: false,
    }
}

#[test]
fn external_identity_lowercases_only_scheme_and_host() {
    let identity = |url: &str| endpoint_identity(&external(url));
    assert_eq!(
        identity(" HTTPS://ARGO.EXAMPLE.COM "),
        "externalHttps:https://argo.example.com"
    );
    assert_eq!(
        identity("https://Argo.Example.com:8443/Argo/Apps"),
        "externalHttps:https://argo.example.com:8443/Argo/Apps"
    );
    assert_ne!(
        identity("https://argo.example.com/Argo"),
        identity("https://argo.example.com/argo")
    );
    assert_eq!(
        identity("https://ARGO.example.com?Q"),
        "externalHttps:https://argo.example.com?Q"
    );
    assert_eq!(
        identity("https://ARGO.example.com#F"),
        "externalHttps:https://argo.example.com#F"
    );
    assert_eq!(
        identity("Argo.Example.com/Argo"),
        "externalHttps:Argo.Example.com/Argo"
    );
    // Saved ids made before lowercased the whole URL; a new identity never equals one.
    assert_ne!(
        identity("https://argo.example.com/argo"),
        "https://argo.example.com/argo"
    );
}

#[test]
fn a_new_connection_reuses_the_saved_profile_with_its_endpoint() {
    let saved = [
        // Made before path case counted: the id lowercases this profile's /Argo path.
        profile(
            "argo:w:c:global:https://argo.example.com/argo",
            external("https://argo.example.com/Argo"),
        ),
        profile("tunnel", tunnel("argocd")),
    ];
    assert_eq!(
        saved_profile_id(&saved, &external("https://ARGO.example.com/Argo")).as_deref(),
        Some("argo:w:c:global:https://argo.example.com/argo")
    );
    // The other server, whose new id could otherwise take the old profile's id.
    assert_eq!(
        saved_profile_id(&saved, &external("https://argo.example.com/argo")),
        None
    );
    assert_eq!(
        saved_profile_id(&saved, &tunnel("ArgoCD")).as_deref(),
        Some("tunnel")
    );
}
