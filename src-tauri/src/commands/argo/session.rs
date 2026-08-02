use crate::models::{AppError, ArgoApplicationRef, ArgoOperationRequest};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub const SESSION_TTL: Duration = Duration::from_secs(5 * 60);
const SERVICE: &str = "KubeCove Argo operation sessions";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OperationSession {
    pub(crate) request: ArgoOperationRequest,
    pub(crate) transport: String,
    pub(crate) issued_at: u64,
    pub(crate) expires_at: u64,
    pub(crate) workspace_id: Option<String>,
    pub(crate) application: ArgoApplicationRef,
    pub(crate) connection_id: Option<String>,
    pub(crate) connection_generation: Option<String>,
}

pub(crate) trait SecureStore: Send + Sync {
    fn read(&self, id: &str) -> Result<Option<String>, ()>;
    fn write(&self, id: &str, value: &str) -> Result<(), ()>;
    fn delete(&self, id: &str) -> Result<(), ()>;
}

struct KeyringStore;
impl SecureStore for KeyringStore {
    fn read(&self, id: &str) -> Result<Option<String>, ()> {
        let entry = key(id).map_err(|_| ())?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(()),
        }
    }
    fn write(&self, id: &str, value: &str) -> Result<(), ()> {
        key(id).map_err(|_| ())?.set_password(value).map_err(|_| ())
    }
    fn delete(&self, id: &str) -> Result<(), ()> {
        match key(id).map_err(|_| ())?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(()),
        }
    }
}

pub(crate) struct SessionStore {
    pub(crate) records: Mutex<HashMap<String, OperationSession>>,
    consumed: Mutex<std::collections::HashSet<String>>,
    secure: Arc<dyn SecureStore>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            consumed: Mutex::new(std::collections::HashSet::new()),
            secure: Arc::new(KeyringStore),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Consumed {
    consumed_at: u64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn key(id: &str) -> Result<keyring::Entry, AppError> {
    keyring::Entry::new(SERVICE, id).map_err(|_| {
        AppError::new(
            "native credential storage unavailable",
            "credentialUnavailable",
        )
    })
}

fn state_error() -> AppError {
    AppError::new(
        "Argo CD operation state unavailable",
        "argoOperationUnavailable",
    )
}

fn unavailable() -> AppError {
    AppError::new(
        "native credential storage unavailable",
        "credentialUnavailable",
    )
}

impl SessionStore {
    #[cfg(test)]
    fn with_secure(secure: Arc<dyn SecureStore>) -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            consumed: Mutex::new(std::collections::HashSet::new()),
            secure,
        }
    }
}

pub(crate) fn issue(
    store: &SessionStore,
    request: ArgoOperationRequest,
    connection_generation: Option<String>,
) -> Result<(String, OperationSession), AppError> {
    let issued_at = now();
    let session = OperationSession {
        application: request.application.clone(),
        connection_id: request.connection_id.clone(),
        connection_generation,
        workspace_id: request.application.workspace_id.clone(),
        transport: request.transport.clone(),
        request,
        issued_at,
        expires_at: issued_at + SESSION_TTL.as_secs(),
    };
    let id = Uuid::new_v4().to_string();
    let serialized = serde_json::to_string(&session).map_err(|_| state_error())?;
    store
        .secure
        .write(&id, &serialized)
        .map_err(|_| unavailable())?;
    store
        .records
        .lock()
        .map_err(|_| state_error())?
        .insert(id.clone(), session.clone());
    Ok((id, session))
}

#[derive(Debug, Clone)]
pub(crate) struct SessionSnapshot {
    pub(crate) session: OperationSession,
    fingerprint: String,
}

fn load(store: &SessionStore, id: &str) -> Result<(OperationSession, String), AppError> {
    let value = store
        .records
        .lock()
        .map_err(|_| state_error())?
        .get(id)
        .cloned()
        .map(|session| serde_json::to_string(&session).map(|value| (session, value)))
        .transpose()
        .map_err(|_| state_error())?;
    if let Some(value) = value {
        return Ok(value);
    }
    let serialized = store
        .secure
        .read(id)
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            AppError::new(
                "operation session expired or already used",
                "argoOperationUnavailable",
            )
        })?;
    let session = serde_json::from_str(&serialized).map_err(|_| {
        AppError::new(
            "operation session expired or already used",
            "argoOperationUnavailable",
        )
    })?;
    Ok((session, serialized))
}

pub(crate) fn peek(
    store: &SessionStore,
    id: &str,
    now_override: Option<u64>,
) -> Result<SessionSnapshot, AppError> {
    if store
        .consumed
        .lock()
        .map_err(|_| state_error())?
        .contains(id)
    {
        return Err(AppError::new(
            "operation session expired or already used",
            "argoOperationUnavailable",
        ));
    }
    let (session, fingerprint) = load(store, id)?;
    if session.expires_at <= now_override.unwrap_or_else(now) {
        return Err(AppError::new(
            "operation session expired",
            "argoOperationUnavailable",
        ));
    }
    Ok(SessionSnapshot {
        session,
        fingerprint,
    })
}

pub(crate) fn consume(
    store: &SessionStore,
    id: &str,
    expected: &SessionSnapshot,
) -> Result<OperationSession, AppError> {
    let mut records = store.records.lock().map_err(|_| state_error())?;
    let mut consumed = store.consumed.lock().map_err(|_| state_error())?;
    if consumed.contains(id) {
        return Err(AppError::new(
            "operation session expired or already used",
            "argoOperationUnavailable",
        ));
    }
    let (record, fingerprint) = if let Some(record) = records.get(id).cloned() {
        let fingerprint = serde_json::to_string(&record).map_err(|_| state_error())?;
        if store
            .secure
            .read(id)
            .map_err(|_| unavailable())?
            .is_some_and(|value| value != fingerprint)
        {
            return Err(AppError::new(
                "operation session changed since review",
                "argoOperationUnavailable",
            ));
        }
        (record, fingerprint)
    } else {
        let serialized = store
            .secure
            .read(id)
            .map_err(|_| unavailable())?
            .ok_or_else(|| {
                AppError::new(
                    "operation session expired or already used",
                    "argoOperationUnavailable",
                )
            })?;
        let record = serde_json::from_str(&serialized).map_err(|_| {
            AppError::new(
                "operation session expired or already used",
                "argoOperationUnavailable",
            )
        })?;
        (record, serialized)
    };
    if fingerprint != expected.fingerprint {
        return Err(AppError::new(
            "operation session changed since review",
            "argoOperationUnavailable",
        ));
    }
    let consumed_at = now();
    if record.expires_at <= consumed_at {
        return Err(AppError::new(
            "operation session expired",
            "argoOperationUnavailable",
        ));
    }
    records.remove(id);
    consumed.insert(id.to_owned());
    let tombstone = serde_json::to_string(&Consumed { consumed_at }).map_err(|_| state_error())?;
    if store.secure.write(id, &tombstone).is_err() {
        let _ = store.secure.delete(id);
        return Err(unavailable());
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Memory {
        values: Mutex<HashMap<String, String>>,
        fail_write: Mutex<bool>,
        fail_delete: Mutex<bool>,
    }
    impl SecureStore for Memory {
        fn read(&self, id: &str) -> Result<Option<String>, ()> {
            Ok(self.values.lock().unwrap().get(id).cloned())
        }
        fn write(&self, id: &str, value: &str) -> Result<(), ()> {
            if *self.fail_write.lock().unwrap() {
                return Err(());
            }
            self.values.lock().unwrap().insert(id.into(), value.into());
            Ok(())
        }
        fn delete(&self, _id: &str) -> Result<(), ()> {
            if *self.fail_delete.lock().unwrap() {
                Err(())
            } else {
                Ok(())
            }
        }
    }

    fn session() -> OperationSession {
        let request = ArgoOperationRequest {
            transport: "kubernetes".into(),
            ..Default::default()
        };
        OperationSession {
            application: request.application.clone(),
            connection_id: None,
            connection_generation: None,
            workspace_id: None,
            transport: request.transport.clone(),
            request,
            issued_at: 10,
            expires_at: 310,
        }
    }

    #[test]
    fn record_contains_no_credentials_and_expires_in_five_minutes() {
        let value = serde_json::to_value(session()).unwrap();
        assert_eq!(value["expiresAt"], 310);
        assert!(!serde_json::to_string(&value).unwrap().contains("token"));
    }

    #[test]
    fn consume_requires_durable_write_and_is_single_use() {
        let memory = Arc::new(Memory::default());
        let store = SessionStore::with_secure(memory.clone());
        let (id, _) = issue(&store, session().request.clone(), None).unwrap();
        let reviewed = peek(&store, &id, Some(10)).unwrap();
        assert!(consume(&store, &id, &reviewed).is_ok());
        assert!(consume(&store, &id, &reviewed).is_err());
        let (id, _) = issue(&store, session().request, None).unwrap();
        let reviewed = peek(&store, &id, Some(10)).unwrap();
        *memory.fail_write.lock().unwrap() = true;
        assert!(consume(&store, &id, &reviewed).is_err());
        assert!(peek(&store, &id, Some(10)).is_err());
        assert!(memory.values.lock().unwrap().contains_key(&id));
    }

    #[test]
    fn consume_rechecks_expiry_after_review() {
        let memory = Arc::new(Memory::default());
        let store = SessionStore::with_secure(memory.clone());
        let (id, _) = issue(&store, session().request, None).unwrap();
        let mut expired = store.records.lock().unwrap().get(&id).unwrap().clone();
        expired.expires_at = now();
        let fingerprint = serde_json::to_string(&expired).unwrap();
        store
            .records
            .lock()
            .unwrap()
            .insert(id.clone(), expired.clone());
        memory
            .values
            .lock()
            .unwrap()
            .insert(id.clone(), fingerprint.clone());
        let reviewed = SessionSnapshot {
            session: expired,
            fingerprint,
        };

        assert_eq!(
            consume(&store, &id, &reviewed).unwrap_err().message,
            "operation session expired"
        );
    }

    #[test]
    fn changed_session_cannot_be_consumed() {
        let memory = Arc::new(Memory::default());
        let store = SessionStore::with_secure(memory);
        let (id, _) = issue(&store, session().request, None).unwrap();
        let reviewed = peek(&store, &id, Some(10)).unwrap();
        store
            .records
            .lock()
            .unwrap()
            .get_mut(&id)
            .unwrap()
            .expires_at += 1;
        assert_eq!(
            consume(&store, &id, &reviewed).unwrap_err().kind,
            "argoOperationUnavailable"
        );
    }

    #[test]
    fn consumed_tombstone_rejects_after_restart() {
        let memory = Arc::new(Memory::default());
        let store = SessionStore::with_secure(memory.clone());
        let (id, _) = issue(&store, session().request, None).unwrap();
        let reviewed = peek(&store, &id, Some(10)).unwrap();
        consume(&store, &id, &reviewed).unwrap();
        let restarted = SessionStore::with_secure(memory);
        assert!(peek(&restarted, &id, Some(10)).is_err());
    }
}
