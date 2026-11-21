use super::{feed::Platform, integrity, invalid, io_error};
use crate::models::AppError;
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
};
use tempfile::NamedTempFile;

pub(super) const BUNDLE: &str = "KubeCove GPUI.app";
const INSTALLER_PREFIX: &str = "installer-";
const SESSION_MARKER: &str = ".session";

/// The installation lock serializes cleanup across processes. A session keeps
/// its directory leased while its installer is still available in Settings.
pub(super) fn stage_installer(temp: &Path) -> Result<(tempfile::TempDir, fs::File), AppError> {
    let temp = temp.canonicalize().map_err(io_error)?;
    if !integrity::protected_staging_parent(&temp) {
        return Err(invalid("Cannot stage GPUI in a shared folder"));
    }
    let parent = temp.join("kubecove-gpui-upgrade");
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    match builder.create(&parent) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        result => result.map_err(io_error)?,
    }
    if !integrity::protected_path(&parent) {
        return Err(invalid(
            "Cannot stage the GPUI installer in a shared temporary folder",
        ));
    }
    cleanup_stale_installer_dirs(&parent).map_err(io_error)?;
    let dir = create_staging(&parent, INSTALLER_PREFIX).map_err(io_error)?;
    let dir = verify_private_staging(dir)?;
    let lease = fs::File::create_new(dir.path().join(SESSION_MARKER)).map_err(io_error)?;
    lease
        .try_lock()
        .map_err(|_| invalid("Cannot reserve the GPUI installer directory"))?;
    Ok((dir, lease))
}

fn cleanup_stale_installer_dirs(parent: &Path) -> io::Result<()> {
    for entry in fs::read_dir(parent)?.flatten() {
        if !entry
            .file_name()
            .as_encoded_bytes()
            .starts_with(INSTALLER_PREFIX.as_bytes())
            || !entry.file_type().is_ok_and(|t| t.is_dir())
        {
            continue;
        }
        let path = entry.path();
        if !integrity::protected_path(&path) {
            continue;
        }
        let _ = remove_stale_installer(&path);
    }
    Ok(())
}

fn remove_stale_installer(path: &Path) -> io::Result<()> {
    let marker = path.join(SESSION_MARKER);
    if !marker.symlink_metadata()?.is_file() {
        return Ok(());
    }
    let lease = fs::File::options().read(true).write(true).open(&marker)?;
    lease.try_lock()?;

    // An open Windows installer can prevent deletion. Keep the marker for the next pass.
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_name() == SESSION_MARKER {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    fs::remove_file(&marker)?;
    drop(lease);
    if let Err(error) = fs::remove_dir(path) {
        let _ = fs::File::create_new(marker);
        return Err(error);
    }
    Ok(())
}

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

/// The caller owns a fresh private staging directory until publication finishes.
pub(super) fn stage(folder: &Path) -> Result<NamedTempFile, AppError> {
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
        .prefix(".download-")
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
    if platform == Platform::Windows {
        // A DOS header alone is not a PE image. Both fixed-width reads must fit.
        file.seek(SeekFrom::Start(0x3c)).map_err(io_error)?;
        file.read_exact(&mut header)
            .map_err(|_| invalid("Missing PE header offset"))?;
        let offset = u64::from(u32::from_le_bytes(header));
        if offset < 0x40 || offset + 4 > file.as_file().metadata().map_err(io_error)?.len() {
            return Err(invalid("Invalid PE header offset"));
        }
        file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
        file.read_exact(&mut header).map_err(io_error)?;
        if &header != b"PE\0\0" {
            return Err(invalid("Missing PE signature"));
        }
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
            let staging = publication_staging(folder, ".kubecove-gpui-")?;
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
pub(super) fn publish_bundle(
    archive: impl Read,
    folder: &Path,
) -> Result<Option<ring::digest::Digest>, AppError> {
    let protected = integrity::protected_path(folder);
    let staging = publication_staging(folder, ".kubecove-gpui-bundle-")?;
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
    let binary = integrity::executable(Platform::Mac, &bundle);
    if !binary.is_file() {
        return Err(invalid("The GPUI bundle is missing its executable"));
    }
    let digest = if protected && integrity::protected_path(&binary) {
        Some(integrity::sha256(
            fs::File::open(binary).map_err(io_error)?,
        )?)
    } else {
        None
    };
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
    Ok(digest)
}

fn publication_staging(folder: &Path, prefix: &str) -> Result<tempfile::TempDir, AppError> {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;

    #[cfg(unix)]
    let dest_dev = folder.metadata().map_err(io_error)?.dev();

    // A shared destination can still accept a rename from a protected ancestor
    // on the same volume. Keep the source outside directories other users can replace.
    for ancestor in folder.ancestors() {
        #[cfg(unix)]
        if ancestor.metadata().map_err(io_error)?.dev() != dest_dev {
            break;
        }
        if !integrity::protected_staging_parent(ancestor) {
            continue;
        }
        match create_staging(ancestor, prefix) {
            Ok(dir) => return verify_private_staging(dir),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem
                ) => {}
            Err(error) => return Err(io_error(error)),
        }
    }
    Err(invalid(
        "KubeCove could not create a private staging directory on the destination volume. \
         Choose a writable installation folder protected from changes by other users.",
    ))
}

fn create_staging(folder: &Path, prefix: &str) -> io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    builder.prefix(prefix).tempdir_in(folder)
}

fn verify_private_staging(dir: tempfile::TempDir) -> Result<tempfile::TempDir, AppError> {
    if !integrity::protected_path(dir.path()) {
        return Err(invalid("Cannot protect the GPUI staging directory"));
    }
    Ok(dir)
}
