//! Same native file operations and import size contract as the Tauri host.
use kubecove_lib::models::{AppError, AppErrorKind};
use std::{fs, io::Read};

// `crates/host/src/workspace_files.rs` defines the web host's import contract.
const WORKSPACE_IMPORT_MAX_BYTES: u64 = 2 * 1024 * 1024;

pub(super) async fn import() -> Result<Option<String>, AppError> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Import KubeCove workspace")
        .add_filter("JSON", &["json"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    read_import(file.path()).map(Some)
}

pub fn read_import(path: &std::path::Path) -> Result<String, AppError> {
    let file = fs::File::open(path).map_err(io_error)?;
    let mut bytes = Vec::new();
    file.take(WORKSPACE_IMPORT_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > WORKSPACE_IMPORT_MAX_BYTES {
        return Err(AppError::new(
            "workspace import JSON must be 2 MiB or smaller",
            AppErrorKind::Validation,
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        AppError::new(
            "workspace import JSON must use UTF-8 encoding",
            AppErrorKind::Validation,
        )
    })
}

pub(super) async fn export(name: String, content: String) -> Result<bool, AppError> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Export KubeCove workspace")
        .set_file_name(name)
        .add_filter("JSON", &["json"])
        .save_file()
        .await
    else {
        return Ok(false);
    };
    fs::write(file.path(), content).map_err(io_error)?;
    Ok(true)
}

fn io_error(error: std::io::Error) -> AppError {
    AppError::new("Could not read or write workspace file", AppErrorKind::Io).with_source(error)
}
