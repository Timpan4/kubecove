use super::*;

#[test]
fn connected_inspection_eagerly_compares_and_redacts_nested_secrets() {
    let inspector = connected_inspector(
        ArgoApplicationRef {
            name: "demo".into(),
            namespace: Some("argocd".into()),
            ..Default::default()
        },
        serde_json::json!({
            "status": {
                "conditions": [{"nested": {"kind": "Secret", "data": {"key": "value"}}}]
            }
        }),
        Some(serde_json::json!({
            "items": [{
                "group": "", "version": "v1", "kind": "Secret", "namespace": "default", "name": "db",
                "targetState": "{\"kind\":\"Secret\",\"data\":{\"password\":\"plaintext\"}}",
                "liveState": {"kind": "Secret", "stringData": {"password": "plaintext"}, "metadata": {"annotations": {
                    "kubectl.kubernetes.io/last-applied-configuration": "{\"data\":{\"password\":\"plaintext\"}}"
                }}},
                "modified": true
            }]
        })),
    );
    let value = serde_json::to_value(&inspector).unwrap();

    assert_eq!(value["transport"], "connected");
    assert_eq!(value["provenance"], "argocd-api");
    assert_eq!(value["comparisons"][0]["exact"], true);
    assert_eq!(
        value["comparisons"][0]["availableActions"],
        serde_json::json!([])
    );
    assert_eq!(
        value["comparisons"][0]["targetState"]["data"]["password"],
        "[REDACTED]"
    );
    assert_eq!(
        value["conditions"][0]["nested"]["data"]["key"],
        "[REDACTED]"
    );
    assert!(!value.to_string().contains("plaintext"));
}

#[test]
fn connected_inspection_without_managed_list_stays_connected_on_status_resources() {
    let inspector = connected_inspector(
        ArgoApplicationRef {
            name: "platform".into(),
            namespace: Some("argocd".into()),
            ..Default::default()
        },
        serde_json::json!({
            "status": {
                "resources": [
                    {"group": "apiextensions.k8s.io", "version": "v1", "kind": "CustomResourceDefinition",
                     "name": "applicationsets.argoproj.io", "status": "Synced", "health": {"status": "Healthy"}},
                    {"version": "v1", "kind": "Secret", "namespace": "argocd", "name": "argocd-secret",
                     "status": "OutOfSync", "requiresPruning": false}
                ],
                "conditions": [{"nested": {"kind": "Secret", "data": {"key": "plaintext"}}}]
            }
        }),
        None,
    );
    let value = serde_json::to_value(&inspector).unwrap();

    assert_eq!(value["transport"], "connected");
    assert_eq!(value["provenance"], "argocd-api");
    assert_eq!(value["comparisons"], serde_json::json!([]));
    assert_eq!(value["resources"][0]["name"], "applicationsets.argoproj.io");
    assert_eq!(value["resources"][0]["health"], "Healthy");
    assert_eq!(value["resources"][1]["status"], "OutOfSync");
    assert!(!value.to_string().contains("plaintext"));
}

#[test]
fn connected_comparison_diff_documents_are_apply_clean() {
    let server_owned = serde_json::json!({
        "apiVersion": "apps/v1", "kind": "Deployment",
        "metadata": {
            "name": "api", "uid": "u1", "resourceVersion": "7", "generation": 1,
            "creationTimestamp": "2026-01-01T00:00:00Z",
            "managedFields": [{"manager": "argocd-controller"}]
        },
        "spec": {"replicas": 1},
        "status": {"readyReplicas": 1}
    });
    let inspector = connected_inspector(
        ArgoApplicationRef {
            name: "demo".into(),
            ..Default::default()
        },
        serde_json::json!({}),
        Some(serde_json::json!({
            "items": [{
                "group": "apps", "version": "v1", "kind": "Deployment", "name": "api",
                "targetState": server_owned.to_string(),
                "liveState": server_owned.to_string(),
                "normalizedLiveState": server_owned.to_string()
            }, {
                "group": "apps", "version": "v1", "kind": "Deployment", "name": "api",
                "targetState": server_owned.to_string(),
                "liveState": server_owned.to_string()
            }, {
                "group": "apps", "version": "v1", "kind": "Deployment", "name": "api",
                "targetState": server_owned.to_string(),
                "liveState": server_owned.to_string(),
                "normalizedLiveState": "null"
            }]
        })),
    );
    assert_eq!(inspector.comparisons.len(), 3);

    for comparison in &inspector.comparisons {
        assert_eq!(
            comparison.live_state.as_ref().unwrap()["status"]["readyReplicas"],
            1
        );
        for document in [&comparison.target_state, &comparison.normalized_live_state] {
            let document = document.as_ref().unwrap();
            assert_eq!(document["metadata"]["name"], "api");
            assert!(document.get("status").is_none());
            for field in [
                "managedFields",
                "uid",
                "resourceVersion",
                "generation",
                "creationTimestamp",
            ] {
                assert!(document["metadata"].get(field).is_none(), "{field} kept");
            }
            assert_eq!(document["spec"]["replicas"], 1);
        }
    }
}

#[test]
fn non_json_secret_state_is_redacted() {
    let yaml = serde_json::json!("kind: Secret\ndata:\n  password: plaintext\n");
    let redacted = state(Some(&yaml), true).unwrap();
    assert_eq!(redacted["data"]["password"], "[REDACTED]");
    let broken = serde_json::json!("kind: Secret\n\tdata: [plaintext");
    assert_eq!(state(Some(&broken), true).unwrap(), "[REDACTED]");
}

#[test]
fn kubernetes_comparisons_do_not_imply_desired_state() {
    let comparison = kubernetes_comparison(ArgoManagedResource {
        kind: Some("Deployment".into()),
        name: Some("api".into()),
        ..Default::default()
    });

    assert_eq!(comparison.exact, Some(false));
    assert_eq!(
        comparison.provenance.as_deref(),
        Some("kubernetes-status-no-diff")
    );
    assert!(comparison.target_state.is_none());
    assert!(comparison.live_state.is_none());
    assert_eq!(comparison.available_actions.len(), 0);
}
