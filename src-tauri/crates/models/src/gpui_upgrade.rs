use serde::Serialize;
use std::path::PathBuf;

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
        path: PathBuf,
        version: String,
        install_kind: GpuiInstallKind,
    },
}
