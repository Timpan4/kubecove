#[cfg(all(unix, test))]
use super::ARGO_CREDENTIAL_SERVICE;
#[cfg(unix)]
use super::{credential_error, credential_io_error, SecureStore, SESSION_SERVICE};
#[cfg(unix)]
use crate::models::AppError;
#[cfg(unix)]
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(unix)]
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::RwLock,
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(unix)]
use uuid::Uuid;

const DATA_FILE: &str = "credentials.enc";
const KEY_FILE: &str = "credentials.key";
const KEY_SIZE: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct EntryKey {
    service: String,
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SealedEntry {
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

type Entries = HashMap<EntryKey, SealedEntry>;

fn entry_key(service: &str, id: &str) -> EntryKey {
    EntryKey {
        service: service.to_owned(),
        id: id.to_owned(),
    }
}

fn associated_data(service: &str, id: &str) -> Vec<u8> {
    let mut value = Vec::with_capacity(8 + service.len() + id.len());
    value.extend_from_slice(&(service.len() as u32).to_be_bytes());
    value.extend_from_slice(service.as_bytes());
    value.extend_from_slice(&(id.len() as u32).to_be_bytes());
    value.extend_from_slice(id.as_bytes());
    value
}

#[cfg(unix)]
pub(super) struct FileCredentialStore {
    directory: PathBuf,
    key: std::sync::Mutex<[u8; KEY_SIZE]>,
    entries: RwLock<()>,
}

#[cfg(unix)]
impl FileCredentialStore {
    pub(super) fn open(directory: &Path) -> Result<Self, AppError> {
        ensure_secure_directory(directory)?;
        let metadata = fs::symlink_metadata(directory).map_err(credential_io_error)?;
        validate_directory(&metadata)?;
        let key_path = directory.join(KEY_FILE);
        let data_path = directory.join(DATA_FILE);
        let key_exists = key_path.exists();
        let data_exists = data_path.exists();
        if data_exists && !key_exists {
            return Err(credential_error());
        }
        let key = if key_exists {
            let key = read_secure_file(&key_path)?;
            let key: [u8; KEY_SIZE] = key.try_into().map_err(|_| credential_error())?;
            if data_exists {
                validate_data_file(&data_path)?;
            }
            key
        } else {
            let mut key = [0u8; KEY_SIZE];
            getrandom::fill(&mut key).map_err(credential_io_error)?;
            write_secure_file_atomic(&key_path, &key)?;
            key
        };
        let store = Self {
            directory: directory.to_path_buf(),
            key: std::sync::Mutex::new(key),
            entries: RwLock::new(()),
        };
        if data_exists {
            store.load_entries()?;
        }
        Ok(store)
    }

    fn load_entries(&self) -> Result<Entries, AppError> {
        let path = self.directory.join(DATA_FILE);
        if !path.exists() {
            return Ok(Entries::new());
        }
        let bytes = read_secure_file(&path)?;
        serde_json::from_slice::<Vec<(EntryKey, SealedEntry)>>(&bytes)
            .map(|entries| entries.into_iter().collect())
            .map_err(credential_io_error)
    }

    fn write_entries(&self, entries: &Entries) -> Result<(), AppError> {
        let bytes =
            serde_json::to_vec(&entries.iter().collect::<Vec<_>>()).map_err(credential_io_error)?;
        write_secure_file_atomic(&self.directory.join(DATA_FILE), &bytes)
    }

    fn decrypt(&self, key: &EntryKey, entry: &SealedEntry) -> Result<String, AppError> {
        let nonce = XNonce::try_from(entry.nonce.as_slice()).map_err(|_| credential_error())?;
        let key_bytes = *self.key.lock().map_err(|_| credential_error())?;
        let cipher = XChaCha20Poly1305::new((&key_bytes).into());
        let plaintext = cipher
            .decrypt(
                &nonce,
                chacha20poly1305::aead::Payload {
                    msg: &entry.ciphertext,
                    aad: &associated_data(&key.service, &key.id),
                },
            )
            .map_err(|_| credential_error())?;
        String::from_utf8(plaintext).map_err(credential_io_error)
    }

    fn seal(&self, key: &EntryKey, value: &str) -> Result<SealedEntry, AppError> {
        let mut nonce = [0u8; 24];
        getrandom::fill(&mut nonce).map_err(credential_io_error)?;
        let nonce = XNonce::try_from(nonce.as_slice()).map_err(|_| credential_error())?;
        let key_bytes = *self.key.lock().map_err(|_| credential_error())?;
        let cipher = XChaCha20Poly1305::new((&key_bytes).into());
        let ciphertext = cipher
            .encrypt(
                &nonce,
                chacha20poly1305::aead::Payload {
                    msg: value.as_bytes(),
                    aad: &associated_data(&key.service, &key.id),
                },
            )
            .map_err(|_| credential_error())?;
        Ok(SealedEntry {
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }

    pub(super) fn purge_expired_sessions(&self) -> Result<(), AppError> {
        let _guard = self.entries.write().map_err(|_| credential_error())?;
        let mut entries = self.load_entries()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut expired = Vec::new();
        for (key, entry) in &entries {
            if key.service != SESSION_SERVICE {
                continue;
            }
            let value: serde_json::Value =
                serde_json::from_str(&self.decrypt(key, entry)?).map_err(credential_io_error)?;
            let deadline = value
                .get("expiresAt")
                .or_else(|| value.get("consumedAt"))
                .and_then(serde_json::Value::as_u64);
            if deadline.is_some_and(|deadline| deadline <= now) {
                expired.push(key.clone());
            }
        }
        for key in expired {
            entries.remove(&key);
        }
        if entries.is_empty() {
            remove_storage_files(&self.directory)?;
        } else {
            self.write_entries(&entries)?;
        }
        Ok(())
    }
}

#[cfg(unix)]
impl SecureStore for FileCredentialStore {
    fn read(&self, service: &str, id: &str) -> Result<Option<String>, AppError> {
        let _guard = self.entries.read().map_err(|_| credential_error())?;
        let entries = self.load_entries()?;
        entries
            .get(&entry_key(service, id))
            .map(|entry| self.decrypt(&entry_key(service, id), entry))
            .transpose()
    }

    fn write(&self, service: &str, id: &str, value: &str) -> Result<(), AppError> {
        let _guard = self.entries.write().map_err(|_| credential_error())?;
        let key_path = self.directory.join(KEY_FILE);
        if !key_path.exists() {
            let mut key = [0u8; KEY_SIZE];
            getrandom::fill(&mut key).map_err(credential_io_error)?;
            write_secure_file_atomic(&key_path, &key)?;
            *self.key.lock().map_err(|_| credential_error())? = key;
        }
        let mut entries = self.load_entries()?;
        let key = entry_key(service, id);
        entries.insert(key.clone(), self.seal(&key, value)?);
        self.write_entries(&entries)
    }

    fn delete(&self, service: &str, id: &str) -> Result<(), AppError> {
        let _guard = self.entries.write().map_err(|_| credential_error())?;
        let mut entries = self.load_entries()?;
        entries.remove(&entry_key(service, id));
        if entries.is_empty() {
            remove_storage_files(&self.directory)
        } else {
            self.write_entries(&entries)
        }
    }
}

#[cfg(unix)]
fn ensure_secure_directory(directory: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    match fs::symlink_metadata(directory) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = directory
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent).map_err(credential_io_error)?;
            }
            match fs::create_dir(directory) {
                Ok(()) => fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                    .map_err(credential_io_error),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                Err(error) => Err(credential_io_error(error)),
            }
        }
        Err(error) => Err(credential_io_error(error)),
    }
}

#[cfg(unix)]
fn validate_directory(metadata: &fs::Metadata) -> Result<(), AppError> {
    use std::os::unix::fs::MetadataExt;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(credential_error());
    }
    Ok(())
}

#[cfg(unix)]
fn validate_file(metadata: &fs::Metadata) -> Result<(), AppError> {
    use std::os::unix::fs::MetadataExt;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(credential_error());
    }
    Ok(())
}

#[cfg(unix)]
fn validate_data_file(path: &Path) -> Result<(), AppError> {
    let metadata = fs::symlink_metadata(path).map_err(credential_io_error)?;
    validate_file(&metadata)
}

#[cfg(unix)]
fn read_secure_file(path: &Path) -> Result<Vec<u8>, AppError> {
    validate_data_file(path)?;
    fs::read(path).map_err(credential_io_error)
}

#[cfg(unix)]
fn write_secure_file_atomic(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    use std::os::unix::fs::OpenOptionsExt;
    let directory = path.parent().ok_or_else(credential_error)?;
    let temp = directory.join(format!(
        ".credentials.{}.{}.tmp",
        std::process::id(),
        Uuid::new_v4()
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(credential_io_error)?;
        file.write_all(bytes).map_err(credential_io_error)?;
        file.sync_all().map_err(credential_io_error)?;
        fs::rename(&temp, path).map_err(credential_io_error)?;
        fs::File::open(directory)
            .and_then(|directory| directory.sync_all())
            .map_err(credential_io_error)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

#[cfg(unix)]
fn remove_storage_files(directory: &Path) -> Result<(), AppError> {
    for name in [DATA_FILE, KEY_FILE] {
        match fs::remove_file(directory.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(credential_io_error(error)),
        }
    }
    fs::File::open(directory)
        .and_then(|directory| directory.sync_all())
        .map_err(credential_io_error)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::{
        fs,
        os::unix::fs::{MetadataExt, PermissionsExt},
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct FileSessionStore(Arc<FileCredentialStore>);

    impl crate::commands::argo::session::SecureStore for FileSessionStore {
        fn read(&self, id: &str) -> Result<Option<String>, AppError> {
            self.0.read(SESSION_SERVICE, id)
        }

        fn write(&self, id: &str, value: &str) -> Result<(), AppError> {
            self.0.write(SESSION_SERVICE, id, value)
        }

        fn delete(&self, id: &str) -> Result<(), AppError> {
            self.0.delete(SESSION_SERVICE, id)
        }
    }

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("kubecove-credentials-{suffix}"));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[test]
    fn rejects_directory_accessible_to_group_or_other_users() {
        let path = temp_dir();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o750)).unwrap();
        assert!(FileCredentialStore::open(&path).is_err());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn creates_missing_data_directory_with_private_permissions() {
        let path = temp_dir().join("new-data");
        FileCredentialStore::open(&path).unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert_eq!(metadata.mode() & 0o777, 0o700);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn rejects_data_and_key_files_accessible_to_group_or_other_users() {
        for file_name in [DATA_FILE, KEY_FILE] {
            let path = temp_dir();
            let store = FileCredentialStore::open(&path).unwrap();
            store.write("svc", "id", "secret").unwrap();
            let file = path.join(file_name);
            fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
            assert!(FileCredentialStore::open(&path).is_err());
            fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn rejects_symlinked_and_incomplete_storage_files() {
        let path = temp_dir();
        let store = FileCredentialStore::open(&path).unwrap();
        store.write("svc", "id", "secret").unwrap();
        let data = path.join(DATA_FILE);
        let linked = path.join("credentials-link");
        fs::rename(&data, &linked).unwrap();
        std::os::unix::fs::symlink(&linked, &data).unwrap();
        assert!(FileCredentialStore::open(&path).is_err());
        fs::remove_dir_all(path).unwrap();

        let path = temp_dir();
        fs::write(path.join(KEY_FILE), [0u8; KEY_SIZE]).unwrap();
        assert!(FileCredentialStore::open(&path).is_err());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn encrypts_each_entry_and_binds_service_and_id_as_aad() {
        let path = temp_dir();
        let store = FileCredentialStore::open(&path).unwrap();
        store.write("service-a", "id-a", "secret").unwrap();
        assert_eq!(
            store.read("service-a", "id-a").unwrap().as_deref(),
            Some("secret")
        );
        assert!(store.read("service-b", "id-a").unwrap().is_none());
        assert!(store.read("service-a", "id-b").unwrap().is_none());
        let raw = fs::read_to_string(path.join(DATA_FILE)).unwrap();
        assert!(!raw.contains("secret"));
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn replacements_are_atomic_private_and_use_fresh_nonces() {
        let path = temp_dir();
        let store = FileCredentialStore::open(&path).unwrap();
        store.write("svc", "id", "first").unwrap();
        let first_nonce = store
            .load_entries()
            .unwrap()
            .remove(&entry_key("svc", "id"))
            .unwrap()
            .nonce;
        store.write("svc", "id", "second").unwrap();
        let entries = store.load_entries().unwrap();
        let second_nonce = &entries[&entry_key("svc", "id")].nonce;
        assert_ne!(&first_nonce, second_nonce);
        assert_eq!(store.read("svc", "id").unwrap().as_deref(), Some("second"));
        for file_name in [DATA_FILE, KEY_FILE] {
            let metadata = fs::symlink_metadata(path.join(file_name)).unwrap();
            assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
            assert_eq!(metadata.mode() & 0o777, 0o600);
        }
        let mut names = fs::read_dir(&path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, vec![DATA_FILE, KEY_FILE]);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn rejects_ciphertext_tampering_and_entry_relabeling() {
        let path = temp_dir();
        let store = FileCredentialStore::open(&path).unwrap();
        store.write("service-a", "id-a", "secret").unwrap();
        let mut map = store.load_entries().unwrap();
        let entry = map.remove(&entry_key("service-a", "id-a")).unwrap();
        map.insert(entry_key("service-b", "id-a"), entry);
        store.write_entries(&map).unwrap();
        assert!(store.read("service-b", "id-a").is_err());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn deleting_last_credential_preserves_another_users_pending_review() {
        let path = temp_dir();
        let store = FileCredentialStore::open(&path).unwrap();
        store
            .write(ARGO_CREDENTIAL_SERVICE, "credential", "secret")
            .unwrap();
        store
            .write(
                SESSION_SERVICE,
                "other-user/review",
                r#"{"expiresAt":4102444800}"#,
            )
            .unwrap();
        store.delete(ARGO_CREDENTIAL_SERVICE, "credential").unwrap();
        let restarted = FileCredentialStore::open(&path).unwrap();
        assert!(restarted
            .read(SESSION_SERVICE, "other-user/review")
            .unwrap()
            .is_some());
        assert!(restarted
            .read(ARGO_CREDENTIAL_SERVICE, "credential")
            .unwrap()
            .is_none());
        restarted
            .delete(SESSION_SERVICE, "other-user/review")
            .unwrap();
        assert!(!path.join(DATA_FILE).exists());
        assert!(!path.join(KEY_FILE).exists());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn expired_and_consumed_sessions_are_purged_at_startup() {
        let path = temp_dir();
        let store = FileCredentialStore::open(&path).unwrap();
        store
            .write(SESSION_SERVICE, "expired", r#"{"expiresAt":1}"#)
            .unwrap();
        store
            .write(SESSION_SERVICE, "consumed", r#"{"consumedAt":1}"#)
            .unwrap();
        store
            .write(SESSION_SERVICE, "live", r#"{"expiresAt":4102444800}"#)
            .unwrap();
        store.purge_expired_sessions().unwrap();
        assert!(store.read(SESSION_SERVICE, "expired").unwrap().is_none());
        assert!(store.read(SESSION_SERVICE, "consumed").unwrap().is_none());
        assert!(store.read(SESSION_SERVICE, "live").unwrap().is_some());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn consumed_operation_session_cannot_be_replayed_from_file_store() {
        use crate::{
            commands::argo::session::{consume, issue, peek, SessionStore},
            models::ArgoOperationRequest,
        };

        let path = temp_dir();
        let file_store = Arc::new(FileCredentialStore::open(&path).unwrap());
        let sessions = SessionStore::with_secure(Arc::new(FileSessionStore(file_store)));
        let request = ArgoOperationRequest {
            transport: "kubernetes".into(),
            ..Default::default()
        };
        let (id, _) = issue(&sessions, request, None, None).unwrap();
        let reviewed = peek(&sessions, &id, None).unwrap();
        consume(&sessions, &id, &reviewed).unwrap();

        let restarted = FileCredentialStore::open(&path).unwrap();
        let restarted_sessions =
            SessionStore::with_secure(Arc::new(FileSessionStore(Arc::new(restarted))));
        assert!(peek(&restarted_sessions, &id, None).is_err());
        fs::remove_dir_all(path).unwrap();
    }
}
