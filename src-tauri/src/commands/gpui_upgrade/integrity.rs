use super::{feed::Platform, io_error};
use crate::models::AppError;
use ring::digest::{Context, Digest, SHA256};
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

pub(super) const BUNDLE_EXECUTABLE: &str = "Contents/MacOS/kubecove-gpui";

pub(super) fn executable(platform: Platform, path: &Path) -> PathBuf {
    if platform == Platform::Mac {
        path.join(BUNDLE_EXECUTABLE)
    } else {
        path.to_owned()
    }
}

pub(super) fn protected_install(platform: Platform, path: &Path) -> bool {
    path.parent().is_some_and(protected_path)
        && protected_path(path)
        && (platform != Platform::Mac || protected_bundle(path))
}

// The main executable can load libraries and other resources from the bundle.
// Check every member without following symlinks or accepting special files.
fn protected_bundle(root: &Path) -> bool {
    let mut pending = vec![root.to_owned()];
    while let Some(path) = pending.pop() {
        let Ok(meta) = fs::symlink_metadata(&path) else {
            return false;
        };
        if !protected(&path, &meta, 0) {
            return false;
        }
        if meta.is_dir() {
            let Ok(entries) = fs::read_dir(&path) else {
                return false;
            };
            for entry in entries {
                let Ok(entry) = entry else {
                    return false;
                };
                pending.push(entry.path());
            }
        } else if !meta.is_file() {
            return false;
        }
    }
    true
}

pub(super) fn sha256(source: impl Read) -> Result<Digest, AppError> {
    let mut reader = BufReader::new(source);
    let mut digest = Context::new(&SHA256);
    loop {
        let bytes = reader.fill_buf().map_err(io_error)?;
        if bytes.is_empty() {
            return Ok(digest.finish());
        }
        digest.update(bytes);
        let count = bytes.len();
        reader.consume(count);
    }
}

/// Check the artifact and each directory that could replace it. An ownership or
/// permissions check that the filesystem cannot answer disables launch.
pub(super) fn protected_path(path: &Path) -> bool {
    protected_path_at_depth(path, 0)
}

pub(super) fn protected_staging_parent(path: &Path) -> bool {
    // A sticky parent permits creating a private child without letting peers replace it.
    protected_path_at_depth(path, 1)
}

fn protected_path_at_depth(path: &Path, offset: usize) -> bool {
    path.is_absolute()
        && path.ancestors().enumerate().all(|(depth, component)| {
            let depth = depth + offset;
            fs::symlink_metadata(component).is_ok_and(|meta| {
                (meta.is_dir() || (depth == 0 && meta.is_file()))
                    && protected(component, &meta, depth)
            })
        })
}

#[cfg(unix)]
fn protected(path: &Path, meta: &fs::Metadata, depth: usize) -> bool {
    use std::os::unix::fs::MetadataExt;
    let trusted_owner = meta.uid() == 0 || meta.uid() == rustix::process::geteuid().as_raw();
    #[cfg(target_os = "macos")]
    if !super::macos_acl::protected(path) {
        return false;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = path;
    // A sticky ancestor such as /tmp protects an already owned private child.
    // A shared destination itself never qualifies.
    trusted_owner && (meta.mode() & 0o022 == 0 || (depth > 0 && meta.mode() & 0o1000 != 0))
}

#[cfg(windows)]
use super::windows_acl::protected;
