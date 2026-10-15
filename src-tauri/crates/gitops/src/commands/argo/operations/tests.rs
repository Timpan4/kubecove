use super::super::session::SecureStore;
use super::payload::resource_action_path;
use super::resolve::{recorded_retry, recorded_sync_payload};
use super::*;
use crate::models::{ArgoConnectionProfile, ArgoServerEndpoint};
use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Mutex},
};

#[derive(Default)]
struct MemorySessionStore(Mutex<HashMap<String, String>>);
impl SecureStore for MemorySessionStore {
    fn read(&self, id: &str) -> Result<Option<String>, AppError> {
        Ok(self.0.lock().unwrap().get(id).cloned())
    }
    fn write(&self, id: &str, value: &str) -> Result<(), AppError> {
        self.0.lock().unwrap().insert(id.into(), value.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), AppError> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}

fn connection(generation: &str) -> Arc<ConnectedArgo> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    Arc::new(ConnectedArgo {
        profile: ArgoConnectionProfile {
            id: "server".into(),
            url: "https://argo.example".into(),
            endpoint: ArgoServerEndpoint::ExternalHttps {
                url: "https://argo.example".into(),
            },
            cluster_context: Some("cluster".into()),
            workspace_id: Some("workspace".into()),
            kubeconfig_source_key: Some("kubeconfigSource=test".into()),
            transport: "connected".into(),
            remember_credential: true,
        },
        token: "token".into(),
        username: None,
        client: reqwest::Client::new(),
        tunnel: None,
        generation: generation.into(),
        instance_id: uuid::Uuid::new_v4().to_string(),
        gate: Arc::new(tokio::sync::Mutex::new(())),
    })
}

fn store() -> super::super::ArgoConnectionStore {
    super::super::ArgoConnectionStore {
        connections: Mutex::new(HashMap::new()),
        sessions: super::super::session::SessionStore::with_secure(Arc::new(
            MemorySessionStore::default(),
        )),
        cleanup_epoch: AtomicU64::default(),
    }
}

fn reviewed(store: &super::super::ArgoConnectionStore) -> (String, SessionSnapshot) {
    let connection = scoped_connection(
        store,
        "server",
        "cluster",
        Some("workspace"),
        Some("kubeconfigSource=test"),
    )
    .unwrap();
    let (id, _) = issue(
        &store.sessions,
        ArgoOperationRequest {
            transport: "connected".into(),
            action: "refresh".into(),
            connection_id: Some("server".into()),
            cluster_context: Some("cluster".into()),
            kubeconfig_env_var: Some("kubeconfigSource=test".into()),
            application: ArgoApplicationRef {
                name: "app".into(),
                workspace_id: Some("workspace".into()),
                ..Default::default()
            },
            ..Default::default()
        },
        Some(connection.generation.clone()),
        Some(connection.instance_id.clone()),
    )
    .unwrap();
    let reviewed = peek(&store.sessions, &id, None).unwrap();
    (id, reviewed)
}

#[tokio::test]
async fn reconnect_with_same_generation_rejects_and_tombstones_reviewed_session() {
    let store = store();
    let first = connection("remembered-generation");
    store
        .replace_connection("server".into(), first.clone())
        .await
        .unwrap();
    let (id, _reviewed) = reviewed(&store);

    let replacement = connection("remembered-generation");
    assert_ne!(first.instance_id, replacement.instance_id);
    store
        .replace_connection("server".into(), replacement)
        .await
        .unwrap();
    let error = run_argo_operation_for_store(
        &store,
        ArgoOperationConfirmation {
            session_id: id.clone(),
            confirmation: id.clone(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.message, "Argo CD connection was replaced");
    assert!(peek(&store.sessions, &id, None).is_err());

    let (id, _reviewed) = reviewed(&store);
    store.remove_connection("server").await.unwrap();
    let error = run_argo_operation_for_store(
        &store,
        ArgoOperationConfirmation {
            session_id: id.clone(),
            confirmation: id.clone(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.message, "Argo CD connection not found");
    assert!(peek(&store.sessions, &id, None).is_err());
}

#[tokio::test]
async fn rejected_lease_tombstones_the_reviewed_session() {
    let store = store();
    let old = connection("old");
    store
        .replace_connection("server".into(), old.clone())
        .await
        .unwrap();
    let (id, reviewed) = reviewed(&store);
    store
        .replace_connection("server".into(), connection("replacement"))
        .await
        .unwrap();

    let error = run_connected_operation(&store, &id, &reviewed, old)
        .await
        .unwrap_err();
    assert_eq!(error.message, "Argo CD connection was replaced");
    assert!(peek(&store.sessions, &id, None).is_err());
}

#[test]
fn operation_paths_encode_application_and_resource_identity() {
    let application = ArgoApplicationRef {
        name: "app/a?#".into(),
        namespace: Some("app ns/&?#".into()),
        project: Some("project/&?#".into()),
        ..Default::default()
    };
    assert_eq!(
        application_path(&application, "/sync", None),
        "/api/v1/applications/app%2Fa%3F%23/sync?appNamespace=app+ns%2F%26%3F%23&project=project%2F%26%3F%23"
    );
    let path = resource_action_path(&ArgoOperationRequest {
        application,
        resources: vec![crate::models::ArgoManagedResource {
            group: Some("apps/&?#".into()),
            version: Some("v1/?.".into()),
            kind: Some("Deployment/#".into()),
            namespace: Some("ns/&?#".into()),
            name: Some("resource/&?#".into()),
            ..Default::default()
        }],
        ..Default::default()
    });
    assert_eq!(
        path,
        "/api/v1/applications/app%2Fa%3F%23/resource/actions?appNamespace=app+ns%2F%26%3F%23&project=project%2F%26%3F%23&group=apps%2F%26%3F%23&version=v1%2F%3F.&kind=Deployment%2F%23&namespace=ns%2F%26%3F%23&resourceName=resource%2F%26%3F%23"
    );
}

#[test]
fn resource_action_requires_complete_server_identity() {
    let complete = crate::models::ArgoManagedResource {
        version: Some("v1".into()),
        kind: Some("ConfigMap".into()),
        name: Some("settings".into()),
        ..Default::default()
    };
    let request = |resource| ArgoOperationRequest {
        transport: "connected".into(),
        action: "resourceAction".into(),
        application: ArgoApplicationRef {
            name: "app".into(),
            ..Default::default()
        },
        resource_action: Some("restart".into()),
        resources: vec![resource],
        ..Default::default()
    };

    assert!(valid(&request(complete.clone())).is_ok());
    for incomplete in [
        crate::models::ArgoManagedResource {
            version: None,
            ..complete.clone()
        },
        crate::models::ArgoManagedResource {
            kind: Some(" ".into()),
            ..complete.clone()
        },
        crate::models::ArgoManagedResource {
            name: Some(String::new()),
            ..complete
        },
    ] {
        let error = valid(&request(incomplete)).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::ArgoOperationUnavailable);
    }
}

#[test]
fn absent_optional_identity_fields_are_not_mismatches() {
    let mut expected = ArgoApplicationRef {
        name: "app".into(),
        ..Default::default()
    };
    let current = json!({"metadata": {"name": "app"}});

    assert!(verify_application_identity(&current, &expected).is_ok());
    expected.uid = Some("reviewed-uid".into());
    assert!(verify_application_identity(&current, &expected).is_err());
}

#[test]
fn kubernetes_identity_includes_object_metadata() {
    let resource = ApiResource::from_gvk(&kube::core::GroupVersionKind::gvk(
        "argoproj.io",
        "v1alpha1",
        "Application",
    ));
    let mut current = DynamicObject::new("app", &resource)
        .within("argocd")
        .data(json!({"spec":{"project":"default"}}));
    current.metadata.uid = Some("uid-1".into());
    current.metadata.resource_version = Some("42".into());
    let expected = ArgoApplicationRef {
        name: "app".into(),
        namespace: Some("argocd".into()),
        uid: Some("uid-1".into()),
        resource_version: Some("42".into()),
        project: Some("default".into()),
        ..Default::default()
    };
    assert!(verify_kubernetes_application_identity(&current, &expected).is_ok());
    current.metadata.resource_version = Some("43".into());
    assert!(verify_kubernetes_application_identity(&current, &expected).is_err());
}

#[test]
fn sync_payload_matches_argocd_request_contract() {
    let request = ArgoOperationRequest {
        action: "sync".into(),
        revision: Some("main".into()),
        force: Some(true),
        resources: vec![crate::models::ArgoManagedResource {
            group: Some("apps".into()),
            version: Some("v1".into()),
            kind: Some("Deployment".into()),
            namespace: Some("default".into()),
            name: Some("web".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_eq!(
        connected_payload(&request),
        json!({"revision":"main","resources":[{"group":"apps","kind":"Deployment","namespace":"default","name":"web"}],"prune":null,"dryRun":null,"syncOptions":{"items":["Force=true","Replace=true"]}})
    );
}

#[test]
fn recorded_retry_is_replayed_as_sync_request() {
    let retry = recorded_retry(&ArgoOperationRequest { action: "retry".into(), ..Default::default() }, json!({"revision":"v2","prune":true,"syncOptions":{"items":["Force=true"]},"resources":[{"group":"apps","version":"v1","kind":"Deployment","namespace":"default","name":"web"}]})).expect("recorded sync");
    assert_eq!(
        connected_payload(&retry),
        json!({"revision":"v2","resources":[{"group":"apps","kind":"Deployment","namespace":"default","name":"web"}],"prune":true,"syncOptions":{"items":["Force=true"]}})
    );
}

#[test]
fn resource_action_contract_is_flat_and_server_scoped() {
    let request = ArgoOperationRequest {
        action: "resourceAction".into(),
        application: ArgoApplicationRef {
            namespace: Some("argocd".into()),
            project: Some("default".into()),
            ..Default::default()
        },
        resource_action: Some("restart".into()),
        resources: vec![crate::models::ArgoManagedResource {
            group: Some("apps".into()),
            version: Some("v1".into()),
            kind: Some("Deployment".into()),
            namespace: Some("default".into()),
            name: Some("web".into()),
            ..Default::default()
        }],
        resource_action_parameters: Some(json!({"replicas":"3","mode":"safe"})),
        ..Default::default()
    };
    assert_eq!(
        connected_payload(&request),
        json!({"appNamespace":"argocd","project":"default","group":"apps","version":"v1","kind":"Deployment","namespace":"default","resourceName":"web","action":"restart","resourceActionParameters":[{"name":"mode","value":"safe"},{"name":"replicas","value":"3"}]})
    );
}

#[test]
fn fallback_sync_uses_crd_option_array() {
    let request = ArgoOperationRequest {
        action: "sync".into(),
        force: Some(true),
        ..Default::default()
    };
    assert_eq!(
        kubernetes_sync_payload(&request)["syncOptions"],
        json!(["Force=true", "Replace=true"])
    );
    assert_eq!(
        connected_payload(&request)["syncOptions"],
        json!({"items":["Force=true", "Replace=true"]})
    );
}

#[test]
fn recorded_sync_allowlists_and_maps_strategy() {
    assert_eq!(
        recorded_sync_payload(json!({"revision":"v1","syncStrategy":{"apply":{"force":true}}}))
            .unwrap(),
        json!({"revision":"v1","resources":[],"syncOptions":{"items":[]},"strategy":{"apply":{"force":true}}})
    );
    assert!(recorded_sync_payload(json!({"source":{"repoURL":"secret"}})).is_err());
    assert!(recorded_sync_payload(json!({"autoHealAttemptsCount":1})).is_err());
}

#[test]
fn fallback_retry_keeps_recorded_crd_sync_strategy() {
    let retry = recorded_retry(
        &ArgoOperationRequest {
            action: "retry".into(),
            transport: "kubernetes".into(),
            ..Default::default()
        },
        json!({"syncStrategy":{"apply":{"force":true}}}),
    )
    .unwrap();
    assert_eq!(
        kubernetes_sync_payload(&retry)["syncStrategy"],
        json!({"apply":{"force":true}})
    );
}

#[test]
fn recorded_sync_rejects_malformed_resources_and_options() {
    assert!(recorded_sync_payload(json!({"resources":{}})).is_err());
    assert!(recorded_sync_payload(json!({"resources":["Deployment"]})).is_err());
    assert!(recorded_sync_payload(json!({"resources":[{"name":4}]})).is_err());
    assert!(recorded_sync_payload(json!({"syncOptions":{}})).is_err());
    assert!(recorded_sync_payload(json!({"syncOptions":{"items":"Force=true"}})).is_err());
    assert!(recorded_sync_payload(json!({"syncOptions":[4]})).is_err());
}

#[test]
fn rollback_payload_is_bound_to_application_and_flags() {
    let request = ArgoOperationRequest {
        action: "rollback".into(),
        history_id: Some(4),
        prune: Some(true),
        dry_run: Some(false),
        application: ArgoApplicationRef {
            name: "app".into(),
            namespace: Some("argo".into()),
            project: Some("default".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        connected_payload(&request),
        json!({"name":"app","appNamespace":"argo","project":"default","id":4,"prune":true,"dryRun":false})
    );
}
