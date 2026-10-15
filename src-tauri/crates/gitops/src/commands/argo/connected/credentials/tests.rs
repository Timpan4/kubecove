use super::*;
use serde_json::json;
use std::{collections::HashMap, sync::Mutex};

#[derive(Default)]
struct MemoryCredentialStore(Mutex<HashMap<String, String>>);
#[test]
fn credential_failure_keeps_native_source_private() {
    let error = credential_error(keyring::Error::NoEntry);
    assert!(std::error::Error::source(&error)
        .unwrap()
        .is::<keyring::Error>());
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        json!({"message":"native credential storage unavailable","kind":"credentialUnavailable"})
    );
}
impl CredentialStore for MemoryCredentialStore {
    fn read(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn write(&self, key: &str, value: &str) -> Result<(), AppError> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<(), AppError> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

fn profile() -> ArgoConnectionProfile {
    ArgoConnectionProfile {
        id: "server".into(),
        url: "https://argo.example/argo-cd".into(),
        endpoint: ArgoServerEndpoint::ExternalHttps {
            url: "https://argo.example/argo-cd".into(),
        },
        cluster_context: Some("cluster".into()),
        workspace_id: Some("workspace".into()),
        kubeconfig_source_key: Some("kubeconfigSource=test".into()),
        transport: "connected".into(),
        remember_credential: true,
    }
}

#[test]
fn service_tunnel_credentials_are_scoped_to_kubeconfig_source() {
    let mut first = profile();
    first.endpoint = ArgoServerEndpoint::ServiceTunnel {
        namespace: "argocd".into(),
        service_name: "argocd-server".into(),
        service_port: 443,
        scheme: "https".into(),
        root_path: None,
        tls_server_name: None,
    };
    let mut second = first.clone();
    second.kubeconfig_source_key = Some("kubeconfigSource=other".into());

    assert_ne!(credential_key(&first), credential_key(&second));
}

#[test]
fn persisted_credential_generation_survives_restart_and_rotates_on_change() {
    let store = MemoryCredentialStore::default();
    let profile = profile();
    let first = Uuid::new_v4().to_string();
    save_credential(&store, &profile, "token-a", first.clone()).unwrap();
    let restarted = load_credential(&store, &profile).unwrap().unwrap();
    assert_eq!(restarted.token, "token-a");
    assert_eq!(restarted.generation, first);
    let rotated = Uuid::new_v4().to_string();
    save_credential(&store, &profile, "token-b", rotated.clone()).unwrap();
    assert_ne!(rotated, restarted.generation);
}

#[test]
fn legacy_raw_token_is_migrated() {
    let store = MemoryCredentialStore::default();
    let profile = profile();
    store
        .0
        .lock()
        .unwrap()
        .insert(credential_key(&profile), "legacy-token".into());

    let migrated = load_credential(&store, &profile).unwrap().unwrap();

    assert_eq!(migrated.token, "legacy-token");
    let stored = store.0.lock().unwrap()[&credential_key(&profile)].clone();
    assert_eq!(
        serde_json::from_str::<StoredCredential>(&stored)
            .unwrap()
            .token,
        "legacy-token"
    );
}

#[test]
fn generation_policy_is_ephemeral_for_submitted_or_tls_credentials() {
    let stored = StoredCredential {
        token: "token".into(),
        generation: "saved-generation".into(),
    };
    assert_eq!(
        connection_generation(Some(&stored), false, true, false, None),
        "saved-generation"
    );
    assert_ne!(
        connection_generation(Some(&stored), true, true, false, None),
        "saved-generation"
    );
    assert_ne!(
        connection_generation(Some(&stored), false, true, true, None),
        "saved-generation"
    );
    assert_ne!(
        connection_generation(Some(&stored), false, true, false, Some(b"ca")),
        "saved-generation"
    );
}

#[test]
fn serialized_credential_contains_no_tls_material() {
    let value = serde_json::to_value(StoredCredential {
        token: "token".into(),
        generation: "generation".into(),
    })
    .unwrap();
    assert_eq!(
        value,
        serde_json::json!({"token": "token", "generation": "generation"})
    );
}
