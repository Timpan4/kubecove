//! MCP is always local, including while a remote runtime is selected.
use super::NativeBackend;
use std::path::PathBuf;

impl NativeBackend {
    /// Only the entry point supplies the desktop config directory. Tests supply private paths.
    pub fn with_mcp_directory(self, directory: PathBuf) -> Self {
        if let Some(startup) = self.mcp.prepare(directory, self.store.clone()) {
            let host = self.mcp.clone();
            self.runtime
                .as_ref()
                .expect("runtime exists until drop")
                .spawn(async move {
                    host.start(startup).await;
                });
        }
        self
    }
}

macro_rules! defaults {
    () => {
        fn mcp_status(&self) -> $crate::backend::Read<kubecove_lib::models::McpStatus> {
            $crate::backend::settings_integrations::unavailable("MCP host unavailable")
        }
        fn set_mcp_settings(
            &self,
            _enabled: bool,
            _port: u16,
        ) -> $crate::backend::Read<kubecove_lib::models::McpStatus> {
            $crate::backend::settings_integrations::unavailable("MCP host unavailable")
        }
        fn reveal_mcp_token(&self) -> $crate::backend::Read<String> {
            $crate::backend::settings_integrations::unavailable("MCP host unavailable")
        }
        fn rotate_mcp_token(&self) -> $crate::backend::Read<kubecove_lib::models::McpStatus> {
            $crate::backend::settings_integrations::unavailable("MCP host unavailable")
        }
    };
}
macro_rules! native {
    () => {
        fn mcp_status(&self) -> $crate::backend::Read<kubecove_lib::models::McpStatus> {
            let host = self.mcp.clone();
            self.read(async move {
                kubecove_lib::mcp::get_mcp_status(kubecove_lib::runtime::State::new(&host)).await
            })
        }
        fn set_mcp_settings(
            &self,
            enabled: bool,
            port: u16,
        ) -> $crate::backend::Read<kubecove_lib::models::McpStatus> {
            let host = self.mcp.clone();
            self.read(async move {
                kubecove_lib::mcp::set_mcp_settings(
                    enabled,
                    port,
                    kubecove_lib::runtime::State::new(&host),
                )
                .await
            })
        }
        fn reveal_mcp_token(&self) -> $crate::backend::Read<String> {
            let host = self.mcp.clone();
            self.read(async move {
                kubecove_lib::mcp::reveal_mcp_token(kubecove_lib::runtime::State::new(&host)).await
            })
        }
        fn rotate_mcp_token(&self) -> $crate::backend::Read<kubecove_lib::models::McpStatus> {
            let host = self.mcp.clone();
            self.read(async move {
                kubecove_lib::mcp::rotate_mcp_token(kubecove_lib::runtime::State::new(&host)).await
            })
        }
    };
}
macro_rules! local {
    () => {
        fn mcp_status(&self) -> $crate::backend::Read<kubecove_lib::models::McpStatus> {
            self.local.mcp_status()
        }
        fn set_mcp_settings(
            &self,
            enabled: bool,
            port: u16,
        ) -> $crate::backend::Read<kubecove_lib::models::McpStatus> {
            self.local.set_mcp_settings(enabled, port)
        }
        fn reveal_mcp_token(&self) -> $crate::backend::Read<String> {
            self.local.reveal_mcp_token()
        }
        fn rotate_mcp_token(&self) -> $crate::backend::Read<kubecove_lib::models::McpStatus> {
            self.local.rotate_mcp_token()
        }
    };
}
pub(super) use {defaults, local, native};
