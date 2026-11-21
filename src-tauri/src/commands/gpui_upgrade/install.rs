use super::{feed::Platform, invalid, io_error};
use crate::models::AppError;
use std::{
    fs,
    io::{self, Read, Seek},
    path::{Component, Path, PathBuf},
};
use tempfile::NamedTempFile;

pub(super) const BUNDLE: &str = "KubeCove GPUI.app";

/// All instances use the same per-user lock while reclaiming the previous download and
/// publishing the result. A crash releases the lock without marking a partial file verified.
pub(super) fn lock_install(cache: &Path) -> Result<fs::File, AppError> {
    fs::create_dir_all(cache).map_err(io_error)?;
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(cache.join("gpui-install.lock"))
        .map_err(io_error)?;
    lock.try_lock()
        .map_err(|_| invalid("Another GPUI installation is in progress"))?;
    Ok(lock)
}

pub(super) fn destination(
    folder: &Path,
    platform: Platform,
    version: &str,
) -> Result<PathBuf, AppError> {
    if folder
        .ancestors()
        .any(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")))
    {
        return Err(invalid("Choose a folder outside an application bundle"));
    }
    let name = match platform {
        Platform::Mac => BUNDLE.into(),
        Platform::Linux => platform.asset(version),
        Platform::Windows => format!("KubeCove-GPUI-{version}-setup.exe"),
    };
    let path = folder.join(name);
    match path.symlink_metadata() {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(path),
        Err(e) => Err(io_error(e)),
        Ok(_) => Err(invalid(
            "The destination already exists. Choose another installation folder.",
        )),
    }
}

/// Called with the installation lock held. Never follow links or recursively clean a directory.
pub(super) fn stage(folder: &Path) -> Result<NamedTempFile, AppError> {
    let name = ".kubecove-gpui-bridge.download";
    let path = folder.join(name);
    match path.symlink_metadata() {
        Ok(meta) if meta.is_file() => fs::remove_file(path).map_err(io_error)?,
        Ok(_) => return Err(invalid("Unsafe GPUI staging path")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_error(e)),
    }
    let mut options = fs::OpenOptions::new();
    options.create_new(true).read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        options.attributes(FILE_ATTRIBUTE_HIDDEN);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    tempfile::Builder::new()
        .prefix(name)
        .rand_bytes(0)
        .make_in(folder, |path| options.open(path))
        .map_err(io_error)
}

pub(super) fn publish_executable(
    mut file: NamedTempFile,
    platform: Platform,
    target: &Path,
) -> Result<(), AppError> {
    let magic: &[u8] = match platform {
        Platform::Linux => b"\x7fELF",
        Platform::Windows => b"MZ",
        Platform::Mac => return Err(invalid("Expected an app bundle")),
    };
    file.rewind().map_err(io_error)?;
    let mut header = [0; 4];
    file.read_exact(&mut header[..magic.len()])
        .map_err(io_error)?;
    if &header[..magic.len()] != magic {
        return Err(invalid("The GPUI file header does not match the platform"));
    }
    #[cfg(unix)]
    if platform == Platform::Linux {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .map_err(io_error)?;
    }
    file.as_file().sync_all().map_err(io_error)?;
    match file.persist_noclobber(target) {
        Ok(_) => {}
        Err(error) if error.error.kind() == io::ErrorKind::CrossesDevices => {
            // The chosen folder may be on a different volume from the user's cache.
            // Copy verified bytes to a private sibling, then publish without replacing any file.
            let folder = target
                .parent()
                .ok_or_else(|| invalid("Missing installation folder"))?;
            let staging = tempfile::Builder::new()
                .prefix(".kubecove-gpui-")
                .tempdir_in(folder)
                .map_err(io_error)?;
            let mut copy = NamedTempFile::new_in(staging.path()).map_err(io_error)?;
            let mut source = error.file;
            source.rewind().map_err(io_error)?;
            io::copy(&mut source, &mut copy).map_err(io_error)?;
            copy.as_file()
                .set_permissions(source.as_file().metadata().map_err(io_error)?.permissions())
                .map_err(io_error)?;
            copy.as_file().sync_all().map_err(io_error)?;
            copy.persist_noclobber(target)
                .map_err(|e| io_error(e.error))?;
        }
        Err(error) => return Err(io_error(error.error)),
    }
    Ok(())
}

/// Release bundles contain regular files and directories. Refuse links and special entries
/// before extraction so an earlier member cannot redirect a subsequent write.
pub(super) fn publish_bundle(archive: impl Read, folder: &Path) -> Result<(), AppError> {
    let staging = tempfile::Builder::new()
        .prefix(".kubecove-gpui-bundle-")
        .tempdir_in(folder)
        .map_err(io_error)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries().map_err(io_error)? {
        let mut entry = entry.map_err(io_error)?;
        let path = entry.path().map_err(io_error)?;
        let mut parts = path.components();
        let kind = entry.header().entry_type();
        let in_bundle = parts.next() == Some(Component::Normal(BUNDLE.as_ref()))
            && parts.all(|part| match part {
                Component::Normal(name) => !Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("app")),
                _ => false,
            });
        if !in_bundle || !(kind.is_file() || kind.is_dir()) {
            return Err(invalid("Unsafe entry in the GPUI app bundle"));
        }
        if !entry.unpack_in(staging.path()).map_err(io_error)? {
            return Err(invalid("GPUI archive entry escapes its staging directory"));
        }
    }
    let bundle = staging.path().join(BUNDLE);
    if !bundle.join("Contents/MacOS/kubecove-gpui").is_file() {
        return Err(invalid("The GPUI bundle is missing its executable"));
    }
    let target = folder.join(BUNDLE);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use rustix::fs::{CWD, RenameFlags, renameat_with};
        renameat_with(CWD, bundle, CWD, target, RenameFlags::NOREPLACE)
            .map_err(|e| io_error(e.into()))?;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        if target.try_exists().map_err(io_error)? {
            return Err(invalid("GPUI is already installed here"));
        }
        fs::rename(bundle, target).map_err(io_error)?;
    }
    Ok(())
}
