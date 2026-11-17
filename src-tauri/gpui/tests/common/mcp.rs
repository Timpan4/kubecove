use kubecove_lib::models::{AppError, AppErrorKind, McpStatus};
use std::sync::Mutex;

/// The settings file as the host last saw it. Mirrors `mcp-dev-mocks.ts`.
#[derive(Clone, Copy, PartialEq)]
enum SettingsFile {
    Loaded,
    /// The startup read failed and the file is still invalid.
    Invalid,
    /// The startup read failed and the user has since fixed the file.
    Fixed,
}

#[derive(serde::Deserialize)]
#[serde(from = "McpStatus")]
pub(super) struct McpFixture {
    status: Mutex<McpStatus>,
    settings_file: Mutex<SettingsFile>,
    rotated: Mutex<bool>,
    requests: Mutex<Vec<String>>,
}
impl From<McpStatus> for McpFixture {
    fn from(status: McpStatus) -> Self {
        Self {
            status: Mutex::new(status),
            settings_file: Mutex::new(SettingsFile::Loaded),
            rotated: Mutex::new(false),
            requests: Mutex::new(Vec::new()),
        }
    }
}
impl McpFixture {
    pub(super) fn status(&self, request: String) -> Result<McpStatus, AppError> {
        self.requests.lock().unwrap().push(request);
        if *self.settings_file.lock().unwrap() != SettingsFile::Loaded {
            return Err(AppError::new(
                "Invalid MCP settings file",
                AppErrorKind::Io,
            ));
        }
        Ok(self.status.lock().unwrap().clone())
    }
    pub(super) fn set(&self, enabled: bool, port: u16) -> Result<McpStatus, AppError> {
        let mut status = self.status.lock().unwrap();
        status.enabled = enabled;
        status.port = port;
        status.running = enabled;
        status.url = enabled.then(|| format!("http://127.0.0.1:{port}/mcp"));
        drop(status);
        self.status(format!("set:{enabled}:{port}"))
    }
    pub(super) fn reload(&self) -> Result<McpStatus, AppError> {
        let mut file = self.settings_file.lock().unwrap();
        if *file == SettingsFile::Fixed {
            *file = SettingsFile::Loaded;
        }
        drop(file);
        self.status("reload".into())
    }
    pub(super) fn reveal(&self) -> String {
        self.requests.lock().unwrap().push("reveal".into());
        if *self.rotated.lock().unwrap() {
            "parity-mcp-rotated-credential"
        } else {
            "parity-mcp-credential"
        }
        .into()
    }
    pub(super) fn rotate(&self) -> Result<McpStatus, AppError> {
        *self.rotated.lock().unwrap() = true;
        self.status("rotate".into())
    }
}
impl super::Fixture {
    // Shared fixture also renders visual tests, which do not inspect requests.
    #[allow(dead_code)]
    pub fn mcp_requests(&self) -> Vec<String> {
        self.mcp.requests.lock().unwrap().clone()
    }
    /// `invalid` stands for a settings file the host could not load at startup; `valid`
    /// for the same file after the user fixes it, which takes effect on reload.
    #[allow(dead_code)]
    pub fn configure_mcp_settings_file(&self, file: &str) {
        let mut state = self.mcp.settings_file.lock().unwrap();
        *state = match (file, *state) {
            ("invalid", _) => SettingsFile::Invalid,
            ("valid", SettingsFile::Invalid) => SettingsFile::Fixed,
            ("valid", current) => current,
            _ => panic!("unknown MCP settings file state {file}"),
        };
    }
}
macro_rules! fixture {
    () => {
        fn mcp_status(&self) -> kubecove_gpui::backend::Read<kubecove_lib::models::McpStatus> {
            let status = self.mcp.status("status".into());
            Box::pin(async move { status })
        }
        fn set_mcp_settings(
            &self,
            enabled: bool,
            port: u16,
        ) -> kubecove_gpui::backend::Read<kubecove_lib::models::McpStatus> {
            let status = self.mcp.set(enabled, port);
            Box::pin(async move { status })
        }
        fn reload_mcp_settings(
            &self,
        ) -> kubecove_gpui::backend::Read<kubecove_lib::models::McpStatus> {
            let status = self.mcp.reload();
            Box::pin(async move { status })
        }
        fn reveal_mcp_token(&self) -> kubecove_gpui::backend::Read<String> {
            let token = self.mcp.reveal();
            Box::pin(async move { Ok(token) })
        }
        fn rotate_mcp_token(
            &self,
        ) -> kubecove_gpui::backend::Read<kubecove_lib::models::McpStatus> {
            let status = self.mcp.rotate();
            Box::pin(async move { status })
        }
    };
}
pub(super) use fixture;
