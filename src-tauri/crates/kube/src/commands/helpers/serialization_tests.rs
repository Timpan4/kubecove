use super::*;
use k8s_openapi::ByteString;
use k8s_openapi::api::core::v1::Secret;
use std::collections::BTreeMap;

#[test]
fn redacts_secret_data_and_string_data() {
    let mut data = BTreeMap::new();
    data.insert("password".to_string(), ByteString(b"super-secret".to_vec()));
    let mut string_data = BTreeMap::new();
    string_data.insert("token".to_string(), "plain-secret".to_string());
    let mut secret = Secret {
        data: Some(data),
        string_data: Some(string_data),
        ..Default::default()
    };

    redact_secret(&mut secret);

    assert_eq!(
        secret.data.as_ref().unwrap().get("password").unwrap().0,
        b"REDACTED".to_vec()
    );
    assert_eq!(
        secret.string_data.as_ref().unwrap().get("token").unwrap(),
        "REDACTED"
    );
}

#[test]
fn audit_secret_last_applied_annotation_is_redacted() {
    let mut secret = Secret::default();
    secret.metadata.annotations = Some(BTreeMap::from([
        (
            "kubectl.kubernetes.io/last-applied-configuration".into(),
            "sensitive".into(),
        ),
        ("note".into(), "keep".into()),
    ]));
    redact_secret(&mut secret);
    let serialized = serde_json::to_string(&secret).unwrap();
    assert!(!serialized.contains("sensitive"));
    assert!(serialized.contains("keep"));
}

#[test]
fn kubectl_mode_strips_managed_fields_only() {
    let mut value = serde_json::json!({
        "apiVersion": "v1",
        "kind": "Service",
        "metadata": {
            "name": "redis",
            "namespace": "argocd",
            "managedFields": [{ "manager": "helm" }],
            "uid": "uid-1",
            "resourceVersion": "10",
            "creationTimestamp": "2026-05-20T12:31:12Z",
            "labels": { "app": "redis" },
            "ownerReferences": [{ "name": "owner" }],
            "finalizers": ["example.com/finalizer"]
        },
        "spec": { "ports": [{ "port": 6379 }] },
        "status": { "loadBalancer": {} }
    });

    normalize_k8s_yaml_value(&mut value, YamlViewMode::Kubectl);

    let metadata = value.get("metadata").unwrap();
    assert!(metadata.get("managedFields").is_none());
    assert_eq!(metadata.get("uid").unwrap(), "uid-1");
    assert!(value.get("status").is_some());
    assert!(metadata.get("ownerReferences").is_some());
    assert!(metadata.get("finalizers").is_some());
}

#[test]
fn apply_clean_mode_strips_server_owned_metadata_and_status() {
    let mut value = serde_json::json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": "api",
            "namespace": "default",
            "managedFields": [],
            "uid": "uid-1",
            "resourceVersion": "10",
            "generation": 4,
            "creationTimestamp": "2026-05-20T12:31:12Z",
            "selfLink": "/api/v1",
            "annotations": { "note": "keep" },
            "labels": { "app": "api" },
            "ownerReferences": [{ "name": "owner" }],
            "finalizers": ["example.com/finalizer"]
        },
        "spec": { "selector": { "matchLabels": { "app": "api" } } },
        "status": { "readyReplicas": 1 }
    });

    normalize_k8s_yaml_value(&mut value, YamlViewMode::ApplyClean);

    let metadata = value.get("metadata").unwrap();
    for key in [
        "managedFields",
        "uid",
        "resourceVersion",
        "generation",
        "creationTimestamp",
        "selfLink",
    ] {
        assert!(metadata.get(key).is_none(), "{key} should be stripped");
    }
    assert!(value.get("status").is_none());
    assert!(metadata.get("annotations").is_some());
    assert!(metadata.get("labels").is_some());
    assert!(metadata.get("ownerReferences").is_some());
    assert!(metadata.get("finalizers").is_some());
    assert!(value.get("spec").is_some());
}

#[test]
fn kyaml_uses_bare_keys_and_quoted_strings() {
    let value = serde_json::json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {
            "name": "app-config",
            "labels": {
                "app.kubernetes.io/name": "api"
            }
        },
        "data": {
            "country": "NO",
            "enabled": "true",
            "answer": 42,
            "flag": true
        }
    });

    let kyaml = serialize_json_value_document(&value, YamlEncoding::Kyaml).unwrap();

    assert!(kyaml.contains("apiVersion: \"v1\""));
    assert!(kyaml.contains("app.kubernetes.io/name: \"api\""));
    assert!(kyaml.contains("country: \"NO\""));
    assert!(kyaml.contains("enabled: \"true\""));
    assert!(kyaml.contains("answer: 42"));
    assert!(kyaml.contains("flag: true"));
    assert!(!kyaml.contains("\"apiVersion\":"));
}

#[test]
fn kyaml_parses_back_to_equivalent_json() {
    let value = serde_json::json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": "api",
            "namespace": "default",
            "managedFields": [{ "manager": "kubecove" }],
            "resourceVersion": "12"
        },
        "spec": {
            "replicas": 2,
            "template": {
                "spec": {
                    "containers": [{ "name": "api", "image": "nginx:1.27" }]
                }
            }
        },
        "status": { "readyReplicas": 1 }
    });
    let mut expected = value.clone();
    normalize_k8s_yaml_value(&mut expected, YamlViewMode::ApplyClean);

    let kyaml = serialize_json_value_document(&expected, YamlEncoding::Kyaml).unwrap();
    let parsed: serde_json::Value =
        serde_json::to_value(serde_yaml::from_str::<serde_yaml::Value>(&kyaml).unwrap()).unwrap();

    assert_eq!(parsed, expected);
    assert!(parsed.get("status").is_none());
    assert!(parsed.pointer("/metadata/resourceVersion").is_none());
}
