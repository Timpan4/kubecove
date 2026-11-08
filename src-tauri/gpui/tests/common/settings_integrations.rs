use super::Fixture;
use kubecove_gpui::backend::{
    AvailableUpdate, KubeconfigSourcesSummary, Read, RemoteConnectionSummary, UpdateStatus,
};
use kubecove_lib::models::{AppError, AppErrorKind};
use std::sync::Mutex;

#[derive(Default)]
pub(super) struct SettingsIntegrations {
    remote: Mutex<Option<RemoteConnectionSummary>>,
    script: Mutex<String>,
    exported: Mutex<String>,
}

impl Fixture {
    pub(super) fn import_workspace_read(&self) -> Read<Option<String>> {
        let content = match self.settings.script.lock().unwrap().as_str() {
            "choose-workspace" => Some(
                include_str!("../../../../src/lib/settings-integration-workspace.json").to_owned(),
            ),
            "choose-unavailable-workspace" => Some(
                include_str!("../../../../src/lib/overview-unavailable-workspace.json").to_owned(),
            ),
            "choose-application-workspace" => Some(
                include_str!("../../../../src/lib/overview-application-workspace.json").to_owned(),
            ),
            "choose-compare-workspace" => Some(
                include_str!("../../../../src/lib/overview-compare-workspace.json").to_owned(),
            ),
            // Reimports the file the last export saved.
            "choose-last-export" => Some(self.settings.exported.lock().unwrap().clone()),
            _ => None,
        };
        Box::pin(async move { Ok(content) })
    }
    pub(super) fn export_workspace_read(&self, content: String) -> Read<bool> {
        *self.settings.exported.lock().unwrap() = content;
        Box::pin(async { Ok(true) })
    }
    /// The last export holds `shortcut`, field for field, on the workspace named `name`.
    // The visual fixture does not inspect a completed export.
    #[allow(dead_code)]
    pub fn expect_exported_shortcut(&self, name: &str, shortcut: &serde_json::Value) {
        let document: serde_json::Value =
            serde_json::from_str(&self.settings.exported.lock().unwrap()).unwrap();
        let items = if document["kind"] == "WorkspaceList" {
            document["items"].as_array().unwrap().clone()
        } else {
            vec![document]
        };
        let shortcuts = items
            .iter()
            .find(|item| item["spec"]["displayName"] == name)
            .map(|item| item["spec"]["shortcuts"].clone())
            .unwrap_or_default();
        assert!(
            shortcuts
                .as_array()
                .is_some_and(|shortcuts| shortcuts.contains(shortcut)),
            "Export of {name} lacks shortcut {shortcut}: {shortcuts}"
        );
    }
    // The visual fixture does not inspect a completed export.
    #[allow(dead_code)]
    pub fn expect_workspace_export(&self, name: &str) {
        let document: serde_json::Value =
            serde_json::from_str(&self.settings.exported.lock().unwrap()).unwrap();
        let items = if document["kind"] == "WorkspaceList" {
            document["items"].as_array().unwrap().clone()
        } else {
            vec![document]
        };
        assert!(items.iter().any(|item| {
            item["spec"]["displayName"] == name
                && item["spec"]["scope"]["clusterContext"] == "mock-dev"
                && item["spec"]["scope"]["namespaces"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!("payments"))
        }));
        let item = items
            .iter()
            .find(|item| item["spec"]["displayName"] == name)
            .unwrap();
        let original: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../src/lib/settings-integration-workspace.json"
        ))
        .unwrap();
        for field in [
            "kinds",
            "layout",
            "gitOpsFilter",
            "argoAppFilter",
            "shortcutPreferences",
        ] {
            assert_eq!(
                item["spec"]["scope"][field], original["spec"]["scope"][field],
                "{field} lost on save"
            );
        }
        assert_eq!(
            item["spec"]["portForwards"],
            original["spec"]["portForwards"]
        );
    }
    pub(super) fn initialize_settings_storage(&mut self) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.t")
            .join(format!(
                "fixture-settings-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&directory).unwrap();
        self.settings_file = directory.join("sources.json");
        self.save_settings(&self.kubeconfig.lock().unwrap())
            .unwrap();
    }
    pub(super) fn save_settings(
        &self,
        sources: &KubeconfigSourcesSummary,
    ) -> Result<(), kubecove_lib::models::AppError> {
        let bytes = serde_json::to_vec(sources).unwrap();
        std::fs::write(&self.settings_file, bytes).map_err(|error| {
            kubecove_lib::models::AppError::new(
                "Controlled settings write failed",
                kubecove_lib::models::AppErrorKind::Io,
            )
            .with_source(error)
        })
    }
    // Also compiled into the native and visual fixtures, which do not restart a scenario.
    #[allow(dead_code)]
    pub fn reopen_settings_storage(&self) {
        let sources = serde_json::from_slice(&std::fs::read(&self.settings_file).unwrap()).unwrap();
        *self.kubeconfig.lock().unwrap() = sources;
    }
    // The fixture also compiles into native and visual tests without parity actions.
    #[allow(dead_code)]
    pub fn settings_script(&self, script: &str) {
        *self.settings.script.lock().unwrap() = script.to_owned();
    }
    #[allow(dead_code)]
    pub fn runtime_endpoint(&self) -> String {
        self.settings
            .remote
            .lock()
            .unwrap()
            .as_ref()
            .map_or_else(|| "local".to_owned(), |remote| remote.server_url.clone())
    }
    pub(super) fn remote_status_read(&self) -> Read<Option<RemoteConnectionSummary>> {
        let summary = self.settings.remote.lock().unwrap().clone();
        Box::pin(async move { Ok(summary) })
    }
    pub(super) fn connect_remote_read(&self, url: String) -> Read<RemoteConnectionSummary> {
        let summary = RemoteConnectionSummary {
            server_url: url,
            server_version: "0.13.0".into(),
            session_grace_seconds: 300,
            connected: true,
        };
        *self.settings.remote.lock().unwrap() = Some(summary.clone());
        Box::pin(async move { Ok(summary) })
    }
    pub(super) fn disconnect_remote_read(&self) -> Read<()> {
        self.settings.remote.lock().unwrap().take();
        Box::pin(async { Ok(()) })
    }
    pub(super) fn pick_kubeconfig_read(&self) -> Read<KubeconfigSourcesSummary> {
        let mut sources = self.kubeconfig.lock().unwrap();
        if *self.settings.script.lock().unwrap() == "choose-kubeconfig"
            && !sources
                .paths
                .iter()
                .any(|entry| entry.path == "/controlled/parity.config")
        {
            sources
                .paths
                .push(kubecove_lib::commands::kubeconfig::KubeconfigPathEntry {
                    path: "/controlled/parity.config".into(),
                });
        }
        let sources = sources.clone();
        let saved = self.save_settings(&sources);
        Box::pin(async move {
            saved?;
            Ok(sources)
        })
    }
    pub(super) fn updates_enabled_read(&self) -> bool {
        matches!(
            self.settings.script.lock().unwrap().as_str(),
            "no-update" | "invalid-signature" | "update-available"
        )
    }
    pub(super) fn check_updates_read(&self) -> Read<UpdateStatus> {
        let status = match self.settings.script.lock().unwrap().as_str() {
            "invalid-signature" => UpdateStatus::Available(AvailableUpdate {
                version: "0.14.0".into(),
                notes: None,
            }),
            "update-available" => UpdateStatus::Available(AvailableUpdate {
                version: "0.14.0".into(),
                notes: Some("Fixes topology edges.".into()),
            }),
            _ => UpdateStatus::UpToDate,
        };
        Box::pin(async move { Ok(status) })
    }
    pub(super) fn kubeconfig_sources_read(&self) -> Read<KubeconfigSourcesSummary> {
        let sources = self.kubeconfig.lock().expect("kubeconfig lock").clone();
        self.coverage_read(
            "get_kubeconfig_sources",
            self.timing.read("get_kubeconfig_sources", "", "", sources),
        )
    }
    pub(super) fn set_kubeconfig_env_var_read(
        &self,
        env_var: String,
    ) -> Read<KubeconfigSourcesSummary> {
        if self.coverage_script() == "failed" {
            return self.coverage_read(
                "set_kubeconfig_env_var",
                Box::pin(async {
                    Err(AppError::new(
                        "Missing coverage response",
                        AppErrorKind::NotFound,
                    ))
                }),
            );
        }
        let mut sources = self.kubeconfig.lock().expect("kubeconfig lock");
        let trimmed = env_var.trim();
        sources.kubeconfig_env_var = if trimmed.is_empty() {
            "KUBECONFIG".to_owned()
        } else {
            trimmed.to_owned()
        };
        self.saved_sources_read(sources.clone())
    }
    /// Mirrors the typed command for pasted or typed paths: trimmed, non-empty, unique.
    pub(super) fn add_kubeconfig_paths_read(
        &self,
        paths: Vec<String>,
    ) -> Read<KubeconfigSourcesSummary> {
        let mut sources = self.kubeconfig.lock().expect("kubeconfig lock");
        for path in paths
            .iter()
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        {
            if !sources.paths.iter().any(|entry| entry.path == path) {
                sources
                    .paths
                    .push(kubecove_lib::commands::kubeconfig::KubeconfigPathEntry {
                        path: path.to_owned(),
                    });
            }
        }
        self.saved_sources_read(sources.clone())
    }
    pub(super) fn set_source_labels_read(&self, show: bool) -> Read<KubeconfigSourcesSummary> {
        let mut sources = self.kubeconfig.lock().expect("kubeconfig lock");
        sources.show_source_labels = show;
        self.saved_sources_read(sources.clone())
    }
    /// Orders the known paths as listed. Paths the list omits keep their relative order at the end.
    pub(super) fn reorder_kubeconfig_read(
        &self,
        paths: Vec<String>,
    ) -> Read<KubeconfigSourcesSummary> {
        let mut sources = self.kubeconfig.lock().expect("kubeconfig lock");
        let mut remaining = std::mem::take(&mut sources.paths);
        let mut ordered = Vec::with_capacity(remaining.len());
        for path in &paths {
            if let Some(index) = remaining.iter().position(|entry| &entry.path == path) {
                ordered.push(remaining.remove(index));
            }
        }
        ordered.extend(remaining);
        sources.paths = ordered;
        self.saved_sources_read(sources.clone())
    }
    fn saved_sources_read(
        &self,
        sources: KubeconfigSourcesSummary,
    ) -> Read<KubeconfigSourcesSummary> {
        let saved = self.save_settings(&sources);
        Box::pin(async move {
            saved?;
            Ok(sources)
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.settings_file);
        if let Some(directory) = self.settings_file.parent() {
            let _ = std::fs::remove_dir(directory);
        }
    }
}
