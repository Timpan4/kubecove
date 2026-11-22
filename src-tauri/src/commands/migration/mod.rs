mod sanitize;
#[cfg(test)]
mod tests;

use crate::models::{AppError, AppErrorKind, MigrationInput};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use tauri::State;

/// Set during startup from the exact directory used for kubeconfig sources and MCP.
pub struct MigrationDirectory(pub PathBuf);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Snapshot {
    schema_version: u8,
    source_version: String,
    exported_at: String,
    entries: Value,
}

fn invalid() -> AppError {
    AppError::new(
        "Could not read stored migration preferences",
        AppErrorKind::Validation,
    )
}
fn io_error(error: io::Error) -> AppError {
    AppError::new("Could not save the migration snapshot", AppErrorKind::Io).with_source(error)
}

fn write(directory: &Path, input: MigrationInput) -> Result<bool, AppError> {
    let mut entries = json!({});
    for (key, raw, project) in [
        (
            "kubecove-settings",
            input.settings,
            sanitize::settings as fn(&Value) -> Value,
        ),
        (
            "kubecove-workspaces",
            input.workspaces,
            sanitize::workspaces,
        ),
    ] {
        if let Some(raw) = raw {
            let envelope: Value = serde_json::from_str(&raw).map_err(|_| invalid())?;
            let state = envelope
                .get("state")
                .filter(|s| s.is_object())
                .ok_or_else(invalid)?;
            if envelope
                .get("version")
                .is_some_and(|v| v.as_u64() != Some(0))
            {
                return Err(invalid());
            }
            entries[key] = json!({"version":0, "state":project(state)});
        }
    }
    fs::create_dir_all(directory).map_err(io_error)?;
    let path = directory.join("tauri-migration.json");
    match fs::read(&path) {
        Ok(bytes) => {
            if serde_json::from_slice::<Snapshot>(&bytes).is_ok_and(|previous| {
                previous.schema_version == 1
                    && previous.entries == entries
                    && previous.source_version == env!("CARGO_PKG_VERSION")
            }) {
                return Ok(false);
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(error)),
    }
    let snapshot = Snapshot {
        schema_version: 1,
        source_version: env!("CARGO_PKG_VERSION").into(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        entries,
    };
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(io_error)?;
    serde_json::to_writer(&mut file, &snapshot).map_err(|error| {
        if error.is_io() {
            io_error(error.into())
        } else {
            AppError::new(
                "Could not serialize the migration snapshot",
                AppErrorKind::Validation,
            )
            .with_source(error)
        }
    })?;
    file.as_file().sync_all().map_err(io_error)?;
    file.persist(path).map_err(|e| io_error(e.error))?;
    Ok(true)
}

pub async fn export_tauri_migration_snapshot(
    input: MigrationInput,
    state: State<'_, MigrationDirectory>,
) -> Result<(), AppError> {
    let directory = state.0.clone();
    tokio::task::spawn_blocking(move || write(&directory, input))
        .await
        .map_err(|_| AppError::new("Migration snapshot task stopped", AppErrorKind::Internal))??;
    Ok(())
}
