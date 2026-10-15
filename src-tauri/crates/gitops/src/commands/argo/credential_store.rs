use crate::{
    commands::helpers::session_scope::SessionScope,
    models::{AppError, AppErrorKind},
};
#[cfg(not(feature = "desktop"))]
use serde::{Deserialize, Serialize};
#[cfg(not(feature = "desktop"))]
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
#[cfg(not(feature = "desktop"))]
use uuid::Uuid;

pub(crate) const ARGO_CREDENTIAL_SERVICE: &str = "KubeCove Argo CD";
pub(crate) const SESSION_SERVICE: &str = "KubeCove Argo operation sessions";
#[cfg(not(feature = "desktop"))]
const PROBE_SERVICE: &str = "KubeCove Credential Store Probe";
#[cfg(not(feature = "desktop"))]
const FILE_STORE_KIND: &str = "encryptedFile";
#[cfg(not(feature = "desktop"))]
const NATIVE_STORE_KIND: &str = "native";

pub(crate) trait SecureStore: Send + Sync {
    fn read(&self, service: &str, id: &str) -> Result<Option<String>, AppError>;
    fn write(&self, service: &str, id: &str, value: &str) -> Result<(), AppError>;
    fn delete(&self, service: &str, id: &str) -> Result<(), AppError>;
}

#[cfg(not(feature = "desktop"))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStoreDescription {
    pub kind: String,
    pub description: String,
}

#[cfg(not(feature = "desktop"))]
fn description(kind: &str, text: &str) -> CredentialStoreDescription {
    CredentialStoreDescription {
        kind: kind.to_owned(),
        description: text.to_owned(),
    }
}

fn credential_error() -> AppError {
    AppError::new(
        "native credential storage unavailable",
        AppErrorKind::CredentialUnavailable,
    )
}

fn credential_io_error(error: impl std::error::Error + Send + Sync + 'static) -> AppError {
    credential_error().with_source(error)
}

#[derive(Default)]
pub(crate) struct KeyringStore;

impl SecureStore for KeyringStore {
    fn read(&self, service: &str, id: &str) -> Result<Option<String>, AppError> {
        let entry = keyring::Entry::new(service, id).map_err(credential_io_error)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(credential_io_error(error)),
        }
    }

    fn write(&self, service: &str, id: &str, value: &str) -> Result<(), AppError> {
        keyring::Entry::new(service, id)
            .map_err(credential_io_error)?
            .set_password(value)
            .map_err(credential_io_error)
    }

    fn delete(&self, service: &str, id: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(service, id).map_err(credential_io_error)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(credential_io_error(error)),
        }
    }
}

static ACTIVE_STORE: OnceLock<RwLock<Arc<dyn SecureStore>>> = OnceLock::new();
#[cfg(not(feature = "desktop"))]
static ACTIVE_DESCRIPTION: OnceLock<RwLock<CredentialStoreDescription>> = OnceLock::new();

fn store_slot() -> &'static RwLock<Arc<dyn SecureStore>> {
    ACTIVE_STORE.get_or_init(|| RwLock::new(Arc::new(KeyringStore)))
}

#[cfg(not(feature = "desktop"))]
fn description_slot() -> &'static RwLock<CredentialStoreDescription> {
    ACTIVE_DESCRIPTION.get_or_init(|| {
        RwLock::new(description(
            NATIVE_STORE_KIND,
            "Credentials are stored in native credential storage.",
        ))
    })
}

pub(crate) fn active_store() -> Arc<dyn SecureStore> {
    let store = store_slot().read().map_or_else(
        |_| -> Arc<dyn SecureStore> { Arc::new(KeyringStore) },
        |store| Arc::clone(&store),
    );
    // The server shares one store between its users, and profiles are
    // shared too. Without the namespace, one user's saved Argo CD token
    // would sign another user's requests.
    match SessionScope::current().and_then(|scope| scope.credential_namespace().map(str::to_owned))
    {
        Some(namespace) => Arc::new(NamespacedStore {
            inner: store,
            namespace,
        }),
        None => store,
    }
}

struct NamespacedStore {
    inner: Arc<dyn SecureStore>,
    namespace: String,
}

impl NamespacedStore {
    // The length prefix keeps two namespaces from producing the same id.
    fn id(&self, id: &str) -> String {
        format!("{}:{}{id}", self.namespace.len(), self.namespace)
    }
}

impl SecureStore for NamespacedStore {
    fn read(&self, service: &str, id: &str) -> Result<Option<String>, AppError> {
        self.inner.read(service, &self.id(id))
    }

    fn write(&self, service: &str, id: &str, value: &str) -> Result<(), AppError> {
        self.inner.write(service, &self.id(id), value)
    }

    fn delete(&self, service: &str, id: &str) -> Result<(), AppError> {
        self.inner.delete(service, &self.id(id))
    }
}

#[cfg(not(feature = "desktop"))]
pub fn active_credential_store_description() -> CredentialStoreDescription {
    description_slot()
        .read()
        .map(|description| description.clone())
        .unwrap_or_else(|_| {
            description(
                NATIVE_STORE_KIND,
                "Credentials are stored in native credential storage.",
            )
        })
}

#[cfg(not(feature = "desktop"))]
pub fn initialize_server_credential_store(
    data_dir: PathBuf,
) -> Result<CredentialStoreDescription, AppError> {
    let native = KeyringStore;
    let probe_id = Uuid::new_v4().to_string();
    let probe = native
        .write(PROBE_SERVICE, &probe_id, "kubecove-probe")
        .and_then(|()| {
            native
                .read(PROBE_SERVICE, &probe_id)?
                .filter(|value| value == "kubecove-probe")
                .ok_or_else(credential_error)
        })
        .and_then(|_| native.delete(PROBE_SERVICE, &probe_id));
    if probe.is_ok() {
        *store_slot().write().map_err(|_| credential_error())? = Arc::new(native);
        *description_slot().write().map_err(|_| credential_error())? = description(
            NATIVE_STORE_KIND,
            "Credentials are stored in native credential storage.",
        );
        return Ok(description(
            NATIVE_STORE_KIND,
            "Credentials are stored in native credential storage.",
        ));
    }

    if let Err(error) = probe {
        let _ = native.delete(PROBE_SERVICE, &probe_id);
        if !is_unavailable_keyring_error(&error) {
            return Err(error);
        }
    }

    #[cfg(unix)]
    {
        let file_store = Arc::new(FileCredentialStore::open(&data_dir)?);
        file_store.purge_expired_sessions()?;
        *store_slot().write().map_err(|_| credential_error())? = file_store;
        *description_slot().write().map_err(|_| credential_error())? = description(
            FILE_STORE_KIND,
            "Credentials are stored in an encrypted file on this host.",
        );
        Ok(description(
            FILE_STORE_KIND,
            "Credentials are stored in an encrypted file on this host.",
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = data_dir;
        Err(credential_error())
    }
}

#[cfg(not(feature = "desktop"))]
fn is_unavailable_keyring_error(error: &AppError) -> bool {
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        if let Some(error) = cause.downcast_ref::<keyring::Error>() {
            return matches!(
                error,
                keyring::Error::NoStorageAccess(_)
                    | keyring::Error::NoDefaultStore
                    | keyring::Error::NotSupportedByStore(_)
                    | keyring::Error::PlatformFailure(_)
            );
        }
        source = cause.source();
    }
    false
}

// The server fallback stays compiled in desktop builds so shared tests cover it.
#[cfg_attr(feature = "desktop", allow(dead_code))]
#[path = "credential_store/file.rs"]
mod file;
#[cfg(all(unix, not(feature = "desktop")))]
use file::FileCredentialStore;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::Mutex};

    #[derive(Default)]
    struct Memory(Mutex<HashMap<(String, String), String>>);

    impl SecureStore for Memory {
        fn read(&self, service: &str, id: &str) -> Result<Option<String>, AppError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .get(&(service.into(), id.into()))
                .cloned())
        }
        fn write(&self, service: &str, id: &str, value: &str) -> Result<(), AppError> {
            self.0
                .lock()
                .unwrap()
                .insert((service.into(), id.into()), value.into());
            Ok(())
        }
        fn delete(&self, service: &str, id: &str) -> Result<(), AppError> {
            self.0.lock().unwrap().remove(&(service.into(), id.into()));
            Ok(())
        }
    }

    #[test]
    fn namespaces_cannot_read_each_others_credentials() {
        let shared: Arc<dyn SecureStore> = Arc::new(Memory::default());
        let store = |namespace: &str| NamespacedStore {
            inner: Arc::clone(&shared),
            namespace: namespace.into(),
        };
        store("alice")
            .write("svc", "profile", "alice-token")
            .unwrap();

        assert_eq!(
            store("alice").read("svc", "profile").unwrap().as_deref(),
            Some("alice-token")
        );
        assert_eq!(store("bob").read("svc", "profile").unwrap(), None);
        assert_eq!(shared.read("svc", "profile").unwrap(), None);

        // Plain concatenation would make both of these "abc".
        store("ab").write("svc", "c", "ab-token").unwrap();
        assert_eq!(store("a").read("svc", "bc").unwrap(), None);
    }
}
