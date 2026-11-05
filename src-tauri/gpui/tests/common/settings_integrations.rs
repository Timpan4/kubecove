use super::Fixture;
use kubecove_gpui::backend::{
    KubeconfigSourcesSummary, Read, RemoteConnectionSummary, UpdateStatus,
};
use std::sync::Mutex;

#[derive(Default)]
pub(super) struct SettingsIntegrations {
    remote: Mutex<Option<RemoteConnectionSummary>>,
    script: Mutex<String>,
    exported: Mutex<String>,
}

impl Fixture {
    pub(super) fn import_workspace_read(&self) -> Read<Option<String>> {
        let content = (*self.settings.script.lock().unwrap() == "choose-workspace").then(|| {
            include_str!("../../../../src/lib/settings-integration-workspace.json").to_owned()
        });
        Box::pin(async move { Ok(content) })
    }
    pub(super) fn export_workspace_read(&self, content: String) -> Read<bool> {
        *self.settings.exported.lock().unwrap() = content;
        Box::pin(async { Ok(true) })
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
            "no-update" | "invalid-signature"
        )
    }
    pub(super) fn check_updates_read(&self) -> Read<UpdateStatus> {
        let status = if *self.settings.script.lock().unwrap() == "invalid-signature" {
            UpdateStatus::Available("0.14.0".into())
        } else {
            UpdateStatus::UpToDate
        };
        Box::pin(async move { Ok(status) })
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
