//! Puts an update in place on each platform. Every platform's helpers compile everywhere, so
//! the shared tests cover them.
use super::{Relaunch, failed};
use kubecove_lib::models::{AppError, AppErrorKind};
use std::{
    ffi::OsString,
    fs, io,
    io::Write as _,
    path::{Path, PathBuf},
};

/// What an update replaces on this machine.
#[derive(Clone, Debug)]
pub enum InstallTarget {
    /// Linux: the AppImage file this process runs from.
    AppImage(PathBuf),
    /// macOS: the `.app` bundle that holds the running executable.
    AppBundle(PathBuf),
    /// Windows: the directory the verified installer is saved to. The installer runs on relaunch.
    Installer(PathBuf),
}

/// The NSIS installer's passive mode, and its restart of the app when it finishes.
const INSTALLER_ARGS: [&str; 2] = ["/P", "/R"];

impl InstallTarget {
    /// The running app's install location. A build started outside its bundle has none.
    pub fn current() -> Result<Self, AppError> {
        #[cfg(target_os = "linux")]
        let target = std::env::var_os("APPIMAGE").map(|path| Self::AppImage(path.into()));
        #[cfg(target_os = "macos")]
        let target = std::env::current_exe()
            .ok()
            .and_then(|executable| bundle_of(&executable))
            .map(Self::AppBundle);
        #[cfg(target_os = "windows")]
        let target = Some(Self::Installer(std::env::temp_dir()));
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let target: Option<Self> = None;
        target.ok_or_else(|| {
            AppError::new(
                "Updates install only when KubeCove runs from its release AppImage, app bundle or installer",
                AppErrorKind::UnsupportedOperation,
            )
        })
    }

    /// Puts verified `bytes` in place. A failure leaves the installed version as it was.
    pub fn install(&self, bytes: &[u8], version: &str) -> Result<Relaunch, AppError> {
        match self {
            Self::AppImage(path) => replace_appimage(path, bytes)
                .map_err(|error| failed("Could not replace the AppImage", &error))
                .map(|()| Relaunch {
                    program: path.clone(),
                    args: Vec::new(),
                }),
            Self::AppBundle(bundle) => replace_bundle(bundle, bytes)
                .map_err(|error| failed("Could not replace the app bundle", &error))
                .map(|()| Relaunch {
                    program: "/usr/bin/open".into(),
                    args: vec!["-n".into(), bundle.clone().into()],
                }),
            Self::Installer(directory) => {
                let installer = directory.join(format!("KubeCove-GPUI-{version}-setup.exe"));
                save_installer(&installer, bytes)
                    .map_err(|error| failed("Could not save the installer", &error))
                    .map(|()| Relaunch {
                        program: installer,
                        args: INSTALLER_ARGS.map(OsString::from).to_vec(),
                    })
            }
        }
    }
}

/// `<Name>.app` for an executable at `<Name>.app/Contents/MacOS/<binary>`.
pub fn bundle_of(executable: &Path) -> Option<PathBuf> {
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then(|| bundle.to_owned())
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// A hidden sibling of `path`, so a rename between the two stays on one file system.
fn sibling(path: &Path, purpose: &str) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| invalid_data("The install path has no file name"))?;
    let mut hidden = OsString::from(".");
    hidden.push(name);
    hidden.push(".");
    hidden.push(purpose);
    Ok(path.with_file_name(hidden))
}

/// Writes a new file and syncs it. It refuses an existing path, so it never writes through a
/// link someone else placed there.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Writes the new AppImage beside the current one and renames it over it, so the path always
/// holds a complete AppImage. The running app keeps its already open file.
fn replace_appimage(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if !bytes.starts_with(b"\x7fELF") {
        return Err(invalid_data("The update is not an AppImage"));
    }
    let permissions = fs::metadata(path)?.permissions();
    let staged = sibling(path, "update")?;
    remove_stale(&staged)?;
    let replaced = write_new(&staged, bytes)
        .and_then(|()| fs::set_permissions(&staged, permissions))
        .and_then(|()| fs::rename(&staged, path));
    if replaced.is_err() {
        let _ = fs::remove_file(&staged);
    }
    replaced
}

/// Unpacks the `.app.tar.gz` beside the installed bundle, then moves the installed bundle
/// aside and the new one into its place. A failed move puts the installed bundle back. The
/// previous bundle is deleted only once the new one is in place.
fn replace_bundle(bundle: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::symlink_metadata(bundle)?;
    let staging = sibling(bundle, "update")?;
    let previous = sibling(bundle, "previous")?;
    remove_stale(&staging)?;
    remove_stale(&previous)?;
    fs::create_dir(&staging)?;
    let replaced = unpack_bundle(bytes, &staging).and_then(|unpacked| {
        fs::rename(bundle, &previous)?;
        fs::rename(&unpacked, bundle).inspect_err(|_| {
            let _ = fs::rename(&previous, bundle);
        })
    });
    let _ = fs::remove_dir_all(&staging);
    if replaced.is_ok() {
        let _ = fs::remove_dir_all(&previous);
    }
    replaced
}

/// Unpacks the archive into `staging` and returns its one `.app` directory. `Archive::unpack`
/// skips entries that would land outside `staging`.
fn unpack_bundle(bytes: &[u8], staging: &Path) -> io::Result<PathBuf> {
    tar::Archive::new(flate2::read::GzDecoder::new(bytes)).unpack(staging)?;
    let mut bundles = Vec::new();
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir()
            && path.extension().is_some_and(|extension| extension == "app")
        {
            bundles.push(path);
        }
    }
    match <[PathBuf; 1]>::try_from(bundles) {
        Ok([bundle]) => Ok(bundle),
        Err(_) => Err(invalid_data("The update does not hold one app bundle")),
    }
}

fn save_installer(installer: &Path, bytes: &[u8]) -> io::Result<()> {
    if !bytes.starts_with(b"MZ") {
        return Err(invalid_data("The update is not a Windows installer"));
    }
    remove_stale(installer)?;
    write_new(installer, bytes)
}

/// Removes what an interrupted earlier install left at `path`, without following a link.
fn remove_stale(path: &Path) -> io::Result<()> {
    let removed = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) => Err(error),
    };
    match removed {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
