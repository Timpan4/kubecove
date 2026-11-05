use kubecove_lib::models::McpStatus;
use std::sync::Mutex;

#[derive(serde::Deserialize)]
#[serde(from = "McpStatus")]
pub(super) struct McpFixture {
    status: Mutex<McpStatus>,
    rotated: Mutex<bool>,
    requests: Mutex<Vec<String>>,
}
impl From<McpStatus> for McpFixture {
    fn from(status: McpStatus) -> Self {
        Self {
            status: Mutex::new(status),
            rotated: Mutex::new(false),
            requests: Mutex::new(Vec::new()),
        }
    }
}
impl McpFixture {
    pub(super) fn status(&self, request: String) -> McpStatus {
        self.requests.lock().unwrap().push(request);
        self.status.lock().unwrap().clone()
    }
    pub(super) fn set(&self, enabled: bool, port: u16) -> McpStatus {
        let mut status = self.status.lock().unwrap();
        status.enabled = enabled;
        status.port = port;
        status.running = enabled;
        status.url = enabled.then(|| format!("http://127.0.0.1:{port}/mcp"));
        drop(status);
        self.status(format!("set:{enabled}:{port}"))
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
    pub(super) fn rotate(&self) -> McpStatus {
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
}
macro_rules! fixture {
    () => {
        fn mcp_status(&self) -> kubecove_gpui::backend::Read<kubecove_lib::models::McpStatus> {
            let status = self.mcp.status("status".into());
            Box::pin(async move { Ok(status) })
        }
        fn set_mcp_settings(
            &self,
            enabled: bool,
            port: u16,
        ) -> kubecove_gpui::backend::Read<kubecove_lib::models::McpStatus> {
            let status = self.mcp.set(enabled, port);
            Box::pin(async move { Ok(status) })
        }
        fn reveal_mcp_token(&self) -> kubecove_gpui::backend::Read<String> {
            let token = self.mcp.reveal();
            Box::pin(async move { Ok(token) })
        }
        fn rotate_mcp_token(
            &self,
        ) -> kubecove_gpui::backend::Read<kubecove_lib::models::McpStatus> {
            let status = self.mcp.rotate();
            Box::pin(async move { Ok(status) })
        }
    };
}
pub(super) use fixture;
