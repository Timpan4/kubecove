use std::fs;

use crate::models::AppError;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

const WORKSPACE_IMPORT_MAX_BYTES: u64 = 2 * 1024 * 1024;

#[tauri::command]
pub fn save_workspace_export_json(
    app: AppHandle,
    file_name: String,
    content: String,
) -> Result<bool, AppError> {
    let Some(file_path) = app
        .dialog()
        .file()
        .set_title("Export KubeCove workspace")
        .set_file_name(file_name)
        .add_filter("JSON", &["json"])
        .blocking_save_file()
    else {
        return Ok(false);
    };

    let path = file_path_to_path(file_path)?;
    fs::write(&path, content).map_err(|err| {
        AppError::new(format!("failed to write workspace export: {err}"), "io")
    })?;
    Ok(true)
}

#[tauri::command]
pub fn pick_workspace_import_json(app: AppHandle) -> Result<Option<String>, AppError> {
    let Some(file_path) = app
        .dialog()
        .file()
        .set_title("Import KubeCove workspace")
        .add_filter("JSON", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };

    let path = file_path_to_path(file_path)?;
    let metadata = fs::metadata(&path).map_err(|err| {
        AppError::new(
            format!("failed to inspect workspace import: {err}"),
            "io",
        )
    })?;
    if metadata.len() > WORKSPACE_IMPORT_MAX_BYTES {
        return Err(AppError::new(
            "workspace import JSON must be 2 MiB or smaller",
            "validation",
        ));
    }
    let content = fs::read_to_string(&path).map_err(|err| {
        AppError::new(format!("failed to read workspace import: {err}"), "io")
    })?;
    Ok(Some(content))
}

fn file_path_to_path(file_path: FilePath) -> Result<std::path::PathBuf, AppError> {
    file_path
        .into_path()
        .map_err(|_| AppError::new("workspace file path is not available", "io"))
}
