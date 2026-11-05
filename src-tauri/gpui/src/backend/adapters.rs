//! Native host adapters. Fixtures override the Backend methods without host effects.
use super::{NativeBackend, Read};
use kubecove_lib::{
    commands,
    models::{AppError, AppErrorKind, ResourceSummary},
};

pub const DOCUMENTATION_URL: &str = "https://github.com/Timpan4/kubecove/wiki";

#[derive(Default, serde::Serialize)]
pub struct BackendDiagnostics {
    pub events: Vec<kubecove_lib::models::BackendDiagnosticEvent>,
    pub caches: Vec<kubecove_lib::models::BackendCacheDiagnosticSnapshot>,
}

pub(super) fn diagnostics(
    backend: &NativeBackend,
    enabled: bool,
    clear: bool,
) -> Read<BackendDiagnostics> {
    let store = backend.store.clone();
    backend.read(async move {
        commands::set_backend_diagnostics_enabled(enabled)?;
        if clear {
            commands::clear_backend_diagnostics(kubecove_lib::runtime::State::new(&store))?;
        }
        Ok(BackendDiagnostics {
            events: commands::get_backend_diagnostics()?,
            caches: commands::get_backend_cache_diagnostics(kubecove_lib::runtime::State::new(
                &store,
            ))?,
        })
    })
}

pub(super) fn open_documentation(backend: &NativeBackend) -> Read<()> {
    backend.read(async {
        let result = tokio::task::spawn_blocking(|| {
            #[cfg(target_os = "linux")]
            let mut command = std::process::Command::new("xdg-open");
            #[cfg(target_os = "macos")]
            let mut command = std::process::Command::new("open");
            #[cfg(target_os = "windows")]
            let mut command = {
                let mut command = std::process::Command::new("cmd");
                command.args(["/C", "start", ""]);
                command
            };
            command.arg(DOCUMENTATION_URL).status()
        })
        .await
        .map_err(|_| AppError::new("Documentation opener task failed", AppErrorKind::Io))?;
        match result {
            Ok(status) if status.success() => Ok(()),
            _ => Err(AppError::new(
                "Could not open documentation",
                AppErrorKind::Io,
            )),
        }
    })
}

pub(super) fn reveal_secret(
    backend: &NativeBackend,
    resource: ResourceSummary,
    key: String,
) -> Read<String> {
    backend.read(async move {
        let namespace = resource
            .namespace
            .filter(|_| resource.kind == "Secret")
            .ok_or_else(|| AppError::new("Select a namespaced Secret", AppErrorKind::Validation))?;
        commands::reveal_secret_data_value(resource.cluster, resource.name, namespace, key, None)
            .await
    })
}
