use k8s_manager_lib::commands::cluster_contexts_from_kubeconfig;
use kube::config::{Context, Kubeconfig, NamedContext};

#[test]
fn maps_kubeconfig_contexts_to_safe_cluster_context_summaries() {
    let mut kubeconfig = Kubeconfig::default();
    kubeconfig.contexts = vec![
        NamedContext {
            name: "kind-dev".into(),
            context: Some(Context {
                cluster: "kind-dev".into(),
                user: Some("dev-user".into()),
                namespace: Some("default".into()),
                extensions: None,
            }),
        },
        NamedContext {
            name: "orphaned".into(),
            context: None,
        },
    ];
    kubeconfig.current_context = Some("kind-dev".into());

    let contexts = cluster_contexts_from_kubeconfig(&kubeconfig);

    assert_eq!(contexts.len(), 2);
    assert_eq!(contexts[0].name, "kind-dev");
    assert_eq!(contexts[0].cluster.as_deref(), Some("kind-dev"));
    assert_eq!(contexts[0].user.as_deref(), Some("dev-user"));
    assert_eq!(contexts[0].namespace.as_deref(), Some("default"));
    assert_eq!(contexts[1].name, "orphaned");
    assert!(contexts[1].cluster.is_none());
    assert!(contexts[1].user.is_none());
    assert!(contexts[1].namespace.is_none());

    let serialized = serde_json::to_value(&contexts).expect("contexts serialize");
    assert_eq!(serialized[0]["cluster"], "kind-dev");
}
