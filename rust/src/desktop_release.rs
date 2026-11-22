//! Release feed validation for the desktop updater.
use crate::models::{AppError, AppErrorKind};
use semver::Version;
use serde_json::Value;

#[cfg(test)]
mod tests;

pub const RELEASES_URL: &str = "https://github.com/Timpan4/kubecove/releases";
pub const CONSOLIDATED_MIN: Version = Version::new(0, 16, 0);
// Same 2 MiB budget as remote GitOps JSON reads.
pub const MANIFEST_BYTES: usize = 2 * 1024 * 1024;

fn invalid(message: &str) -> AppError {
    AppError::new(message, AppErrorKind::Validation)
}

pub async fn read_manifest(mut response: reqwest::Response) -> Result<Value, AppError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::new("Could not read the release manifest", AppErrorKind::Network))?
    {
        if chunk.len() > MANIFEST_BYTES - bytes.len() {
            return Err(invalid("The release manifest is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid release manifest"))
}

pub fn asset_name(prefix: &str, version: &Version, platform: &str) -> Option<String> {
    let suffix = match platform {
        "linux-x86_64" => "x86_64.AppImage",
        "windows-x86_64" => "x64-setup.exe",
        "darwin-aarch64" => "aarch64.app.tar.gz",
        _ => return None,
    };
    Some(format!("{prefix}_{version}_{suffix}"))
}

pub fn version(manifest: &Value) -> Result<Version, AppError> {
    manifest["version"]
        .as_str()
        .and_then(|s| Version::parse(s.strip_prefix('v').unwrap_or(s)).ok())
        .ok_or_else(|| invalid("Invalid release version"))
}

/// Static manifests have no identity fields by default. If a producer supplies them,
/// they must agree with the channel, including on a per-platform override.
pub fn validate_product(
    manifest: &Value,
    platform: &str,
    name: &str,
    id: &str,
) -> Result<(), AppError> {
    for record in [manifest, &manifest["platforms"][platform]] {
        for (field, expected) in [
            ("bundleId", id),
            ("identifier", id),
            ("bundleIdentifier", id),
            ("productName", name),
            ("appName", name),
        ] {
            if record
                .get(field)
                .is_some_and(|value| value.as_str() != Some(expected))
            {
                return Err(invalid(
                    "The release manifest names a different desktop product",
                ));
            }
        }
    }
    Ok(())
}

pub fn matches_asset(url: &str, name: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https" && url.path_segments().and_then(Iterator::last) == Some(name)
    })
}
