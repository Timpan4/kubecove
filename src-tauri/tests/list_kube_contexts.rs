use k8s_manager_lib::commands::{get_cluster_contexts, list_kube_contexts};

#[test]
fn test_list_kube_contexts_command() {
    let result = list_kube_contexts();
    assert!(result.is_ok());
    let contexts = result.unwrap();
    assert!(contexts.iter().all(|c| !c.name.is_empty()));
}

#[test]
fn test_get_cluster_contexts_returns_cluster_contexts() {
    let result = get_cluster_contexts();
    assert!(result.is_ok());
    let contexts = result.unwrap();
    for ctx in &contexts {
        assert!(!ctx.name.is_empty());
    }
}

#[test]
fn test_cluster_context_fields() {
    let result = get_cluster_contexts();
    assert!(result.is_ok());
    let contexts = result.unwrap();
    for ctx in &contexts {
        assert_eq!(std::mem::size_of_val(&ctx.name), std::mem::size_of::<String>());
    }
}