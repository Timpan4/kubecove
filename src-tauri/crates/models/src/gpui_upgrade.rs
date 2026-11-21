use serde::{Deserialize, Serialize};

/// Local storage envelopes from the Tauri webview, projected before writing.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationInput {
    pub settings: Option<String>,
    pub workspaces: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GpuiInstallKind {
    AppImage,
    AppBundle,
    WindowsInstaller,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GpuiUpgradeInfo {
    Consolidated {
        version: String,
    },
    Unsupported {
        target: String,
        message: String,
    },
    Unavailable {
        target: String,
    },
    Available {
        target: String,
        version: String,
        notes: Option<String>,
        published_at: Option<String>,
        asset_name: String,
        install_kind: GpuiInstallKind,
    },
    Installed {
        path: String,
        can_launch: bool,
        version: String,
        install_kind: GpuiInstallKind,
    },
}
