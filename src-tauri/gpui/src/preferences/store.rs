//! All filesystem access is explicit. Default stores are memory-only.
use super::{
    Document,
    tauri_migration::{self, IMPORTED_FILE, MIGRATION_FILE},
};
use crate::workspaces::SavedKind;
use kubecove_lib::models::{AppError, AppErrorKind};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Default)]
pub struct Store {
    directory: Option<PathBuf>,
    memory: Mutex<Document>,
    legacy_file: Option<PathBuf>,
    migration_diagnostic: Mutex<Option<String>>,
}
impl Store {
    pub fn file(directory: PathBuf) -> Self {
        Self {
            directory: Some(directory),
            memory: Mutex::default(),
            legacy_file: None,
            migration_diagnostic: Mutex::default(),
        }
    }
    pub fn set_legacy_file(&mut self, path: PathBuf) {
        self.legacy_file = Some(path);
    }
    pub fn last_migration_diagnostic(&self) -> Option<String> {
        self.migration_diagnostic
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
    }
    fn set_migration_diagnostic(&self, message: Option<String>) {
        if let Some(ref text) = message {
            eprintln!("KubeCove Tauri migration: {text}");
        }
        if let Ok(mut slot) = self.migration_diagnostic.lock() {
            *slot = message;
        }
    }
    pub fn clear_memory(&self) -> Result<(), AppError> {
        if self.directory.is_some() {
            return Err(invalid());
        }
        *self.memory.lock().map_err(|_| invalid())? = Document::default();
        Ok(())
    }
    pub fn load(&self) -> Result<Document, AppError> {
        match &self.directory {
            None => Ok(self.memory.lock().map_err(|_| invalid())?.clone()),
            Some(directory) => {
                if directory.join(MIGRATION_FILE).exists() {
                    return self.load_with_tauri_migration(directory);
                }
                self.load_unlocked(directory).map(|(document, _)| document)
            }
        }
    }
    fn load_unlocked(&self, directory: &Path) -> Result<(Document, bool), AppError> {
        match fs::read(directory.join("live-sessions.json")) {
            Ok(bytes) => Ok((decode(&bytes)?, true)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => match &self.legacy_file {
                Some(path) => match fs::read(path) {
                    Ok(bytes) => Ok((decode(&bytes)?, true)),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        Ok((Document::default(), false))
                    }
                    Err(_) => Err(io_error()),
                },
                None => Ok((Document::default(), false)),
            },
            Err(_) => Err(io_error()),
        }
    }
    fn load_with_tauri_migration(&self, directory: &Path) -> Result<Document, AppError> {
        let mut memory = self.memory.lock().map_err(|_| invalid())?;
        fs::create_dir_all(directory).map_err(|_| io_error())?;
        let _lock = match acquire_lock(directory) {
            Ok(lock) => lock,
            Err(_) => {
                self.set_migration_diagnostic(Some(
                    "Skipped Tauri migration snapshot because another instance holds the preferences lock. The file was preserved."
                        .into(),
                ));
                return self.load_unlocked(directory).map(|(document, _)| document);
            }
        };
        let (mut document, has_existing) = self.load_unlocked(directory)?;
        let migration_path = directory.join(MIGRATION_FILE);
        let bytes = match fs::read(&migration_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                *memory = document.clone();
                return Ok(document);
            }
            Err(error) => {
                self.set_migration_diagnostic(Some(format!(
                    "Could not read {MIGRATION_FILE}: {error}. The file was preserved."
                )));
                *memory = document.clone();
                return Ok(document);
            }
        };
        let mut candidate = document.clone();
        let report = match tauri_migration::merge_bytes(&bytes, &mut candidate, has_existing) {
            Ok(report) => report,
            Err(reason) => {
                self.set_migration_diagnostic(Some(format!(
                    "Ignored corrupt {MIGRATION_FILE}: {reason}. The file was preserved."
                )));
                *memory = document.clone();
                return Ok(document);
            }
        };
        if let Err(error) = validate(&candidate).and_then(|()| {
            let value = serde_json::to_value(&candidate).map_err(|_| invalid())?;
            reject_credentials(&value)
        }) {
            self.set_migration_diagnostic(Some(format!(
                "Ignored {MIGRATION_FILE} because merged preferences failed validation: {}. The file was preserved.",
                error.message
            )));
            *memory = document.clone();
            return Ok(document);
        }
        candidate.revision = candidate.revision.checked_add(1).ok_or_else(invalid)?;
        if write_atomic(directory, &candidate).is_err() {
            self.set_migration_diagnostic(Some(format!(
                "Could not persist imported {MIGRATION_FILE} into live-sessions.json. The file was preserved."
            )));
            *memory = document.clone();
            return Ok(document);
        }
        let imported_path = directory.join(IMPORTED_FILE);
        if let Err(error) = fs::rename(&migration_path, &imported_path) {
            self.set_migration_diagnostic(Some(format!(
                "Imported {MIGRATION_FILE} into live-sessions.json, but could not rename it to {IMPORTED_FILE}: {error}."
            )));
        } else if report.skipped > 0 {
            self.set_migration_diagnostic(Some(format!(
                "Imported {MIGRATION_FILE} with {} invalid or unsafe field(s) skipped.",
                report.skipped
            )));
        } else {
            self.set_migration_diagnostic(None);
        }
        document = candidate;
        *memory = document.clone();
        Ok(document)
    }
    pub fn save(&self, mut document: Document) -> Result<Document, AppError> {
        validate(&document)?;
        let mut memory = self.memory.lock().map_err(|_| invalid())?;
        let Some(directory) = &self.directory else {
            if document.revision != memory.revision {
                return Err(conflict());
            }
            document.revision = document.revision.checked_add(1).ok_or_else(invalid)?;
            *memory = document.clone();
            return Ok(document);
        };
        fs::create_dir_all(directory).map_err(|_| io_error())?;
        let _lock = acquire_lock(directory)?;
        // Re-read under the process-shared lock. A corrupt/future document blocks writes.
        if self.load_unlocked(directory)?.0.revision != document.revision {
            return Err(conflict());
        }
        document.revision = document.revision.checked_add(1).ok_or_else(invalid)?;
        write_atomic(directory, &document)?;
        *memory = document.clone();
        Ok(document)
    }
}
fn acquire_lock(directory: &Path) -> Result<File, AppError> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(directory.join("live-sessions.lock"))
        .map_err(|_| io_error())?;
    lock.try_lock().map_err(|_| io_error())?;
    Ok(lock)
}
fn write_atomic(directory: &Path, document: &Document) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(document).map_err(|_| invalid())?;
    let staged = directory.join(format!("live-sessions.{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&staged).map_err(|_| io_error())?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&staged, directory.join("live-sessions.json"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staged);
        return Err(io_error());
    }
    Ok(())
}
fn decode(bytes: &[u8]) -> Result<Document, AppError> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    reject_credentials(&value)?;
    let versioned = value.get("version").is_some();
    let mut document: Document = if !versioned
        && ["presets", "autoStart", "keep"]
            .iter()
            .any(|key| value.get(key).is_some())
    {
        #[derive(Default, serde::Deserialize)]
        #[serde(default, rename_all = "camelCase", deny_unknown_fields)]
        struct Legacy {
            presets: std::collections::HashMap<String, Vec<super::SavedForward>>,
            auto_start: bool,
            keep: bool,
        }
        let legacy: Legacy = serde_json::from_value(value).map_err(|_| invalid())?;
        Document {
            preferences: super::Preferences {
                auto_start_saved_port_forwards: legacy.auto_start,
                keep_live_sessions_on_workspace_switch: legacy.keep,
                ..Default::default()
            },
            saved_forward_presets: legacy.presets,
            ..Default::default()
        }
    } else {
        serde_json::from_value(value).map_err(|_| invalid())?
    };
    // The new-document default is the current version; a stored document
    // without one predates version 2.
    if !versioned {
        document.version = 1;
    }
    match document.version {
        0 | 1 => {
            restore_kind_identities(&mut document);
            document.version = 2;
        }
        2 => {}
        _ => {
            return Err(AppError::new(
                "Native preferences use an unsupported version. The file was preserved.",
                AppErrorKind::Validation,
            ));
        }
    }
    validate(&document)?;
    Ok(document)
}
/// Version 1 saved only kind names. An import kept its document, whose kinds
/// carry the API identity; when the saved names are exactly that document's
/// kinds, take them. Other names stay as they are.
fn restore_kind_identities(document: &mut Document) {
    for workspace in &mut document.workspaces {
        let Some(kinds) = document
            .workspace_documents
            .get(&workspace.id)
            .and_then(|retained| {
                crate::workspaces::transfer::scope_kinds(&retained["spec"]["scope"]).ok()
            })
        else {
            continue;
        };
        if kinds
            .iter()
            .map(SavedKind::label)
            .eq(workspace.kinds.iter().map(SavedKind::label))
        {
            workspace.kinds = kinds;
        }
    }
}
fn validate(document: &Document) -> Result<(), AppError> {
    if document.version != 2 {
        return Err(invalid());
    }
    let lanes = &document.preferences.resource_kind_lanes;
    let mut seen = std::collections::HashSet::new();
    for kind in lanes.first.iter().chain(&lanes.normal).chain(&lanes.last) {
        if kind.trim().is_empty() || !seen.insert(kind) {
            return Err(invalid());
        }
    }
    if !seen.iter().any(|s| s.as_str() == "*") {
        return Err(invalid());
    }
    let mut ids = std::collections::HashSet::new();
    for workspace in &document.workspaces {
        if workspace.name.trim().is_empty()
            || workspace.context.trim().is_empty()
            || !ids.insert(workspace.id)
            || workspace.id == u64::MAX
            || workspace.updated == u64::MAX
        {
            return Err(invalid());
        }
    }
    for (id, value) in &document.workspace_documents {
        if !ids.contains(id) || crate::workspaces::transfer::parse(&value.to_string()).is_err() {
            return Err(invalid());
        }
        reject_credentials(value)?;
    }
    for (id, entries) in &document.workspace_entry_points {
        if !ids.contains(id)
            || entries
                .pinned
                .iter()
                .chain(&entries.recent)
                .any(|entry| !entry.valid())
        {
            return Err(invalid());
        }
    }
    for profile in &document.preferences.argo_profiles {
        if profile.id.trim().is_empty() {
            return Err(invalid());
        }
        match &profile.endpoint {
            super::Endpoint::ExternalHttps { url } => {
                let parsed = reqwest::Url::parse(url).map_err(|_| invalid())?;
                if parsed.scheme() != "https"
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                    || parsed.query().is_some()
                    || parsed.fragment().is_some()
                {
                    return Err(invalid());
                }
            }
            super::Endpoint::ServiceTunnel {
                namespace,
                service_name,
                service_port,
                scheme,
                ..
            } => {
                if namespace.trim().is_empty()
                    || service_name.trim().is_empty()
                    || *service_port == 0
                    || !matches!(scheme.as_str(), "http" | "https")
                {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}
fn reject_credentials(value: &serde_json::Value) -> Result<(), AppError> {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(
                    key.to_ascii_lowercase().as_str(),
                    "token"
                        | "password"
                        | "secret"
                        | "client-key-data"
                        | "client-certificate-data"
                        | "certificate-authority-data"
                        | "kubeconfig"
                ) {
                    return Err(invalid());
                }
                reject_credentials(value)?;
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                reject_credentials(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn invalid() -> AppError {
    AppError::new(
        "Invalid native preferences. The existing file was preserved.",
        AppErrorKind::Validation,
    )
}
fn io_error() -> AppError {
    AppError::new(
        "Could not read or save native preferences. Changes are unsaved.",
        AppErrorKind::Io,
    )
}
fn conflict() -> AppError {
    AppError::new(
        "Native preferences changed in another instance. Reopen the app before saving.",
        AppErrorKind::Validation,
    )
}
