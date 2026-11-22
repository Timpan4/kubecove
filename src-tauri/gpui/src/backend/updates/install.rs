//! Puts an update in place on each platform. Every platform's helpers compile everywhere, so
//! the shared tests cover them.
//!
//! A download is written to a staged file beside the install location as it arrives. The
//! staged file is not executed, renamed or unpacked until the caller has verified it, and it is
//! removed on every path that does not install it.
use super::{Relaunch, failed};
use kubecove_lib::models::{AppError, AppErrorKind};
use std::{
    ffi::OsString,
    fs,
    io::{self, Read as _, Seek as _, Write as _},
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

    /// Creates the empty file a download is written to. It sits beside the install location, so
    /// the final rename stays on one file system. The installed version is not touched.
    pub fn stage(&self) -> Result<Staged, AppError> {
        let staged = self.staged_path().and_then(|path| {
            remove_stale(&path)?;
            let file = create_new(&path)?;
            Ok(Staged {
                target: self.clone(),
                path,
                file,
                moved: false,
            })
        });
        staged.map_err(|error| failed("Could not stage the update", &error))
    }

    /// The staged file's path. A missing install location fails here, before any download.
    fn staged_path(&self) -> io::Result<PathBuf> {
        match self {
            Self::AppImage(path) => {
                fs::metadata(path)?;
                sibling(path, "update")
            }
            Self::AppBundle(bundle) => {
                fs::symlink_metadata(bundle)?;
                sibling(bundle, "download")
            }
            // The name holds no version, because the version is not verified yet, and it does
            // not end in `.exe`, so an unverified file never sits at an installer's name.
            Self::Installer(directory) => {
                sibling(&directory.join("KubeCove-GPUI-setup.exe"), "download")
            }
        }
    }
}

/// A download's staged file. Dropping it without a completed install removes the file, so a
/// failed, refused or abandoned download leaves nothing behind.
pub struct Staged {
    target: InstallTarget,
    path: PathBuf,
    file: fs::File,
    /// The install renamed the file into place, so nothing is left at `path` to remove.
    moved: bool,
}

impl Staged {
    pub fn write(&mut self, chunk: &[u8]) -> Result<(), AppError> {
        self.file
            .write_all(chunk)
            .map_err(|error| failed("Could not save the update", &error))
    }

    /// Puts the staged download in place. Call it only once the download has ended and its
    /// signature names `version`. A failure leaves the installed version as it was.
    pub fn install(mut self, version: &str) -> Result<Relaunch, AppError> {
        match &self.target {
            InstallTarget::AppImage(path) => {
                replace_appimage(path, &self.path, &mut self.file)
                    .map_err(|error| failed("Could not replace the AppImage", &error))?;
                self.moved = true;
                Ok(Relaunch {
                    program: path.clone(),
                    args: Vec::new(),
                })
            }
            InstallTarget::AppBundle(bundle) => {
                replace_bundle(bundle, &mut self.file, |from, to| fs::rename(from, to))
                    .map_err(|error| failed("Could not replace the app bundle", &error))
                    .map(|()| Relaunch {
                        program: "/usr/bin/open".into(),
                        args: vec!["-n".into(), bundle.clone().into()],
                    })
            }
            InstallTarget::Installer(directory) => {
                let installer = directory.join(format!("KubeCove-GPUI-{version}-setup.exe"));
                save_installer(&installer, &self.path, &mut self.file)
                    .map_err(|error| failed("Could not save the installer", &error))?;
                self.moved = true;
                Ok(Relaunch {
                    program: installer,
                    args: INSTALLER_ARGS.map(OsString::from).to_vec(),
                })
            }
        }
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.moved {
            let _ = fs::remove_file(&self.path);
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

/// Opens a new file for writing and reading back. It refuses an existing path, so it never
/// writes through a link someone else placed there.
fn create_new(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
}

/// Whether the file's first bytes are `magic`.
fn starts_with(file: &mut fs::File, magic: &[u8]) -> io::Result<bool> {
    let mut head = vec![0; magic.len()];
    file.rewind()?;
    match file.read_exact(&mut head) {
        Ok(()) => Ok(head == magic),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

/// Gives the staged AppImage the current one's permissions and renames it over it, so the path
/// always holds a complete AppImage. The running app keeps its already open file.
fn replace_appimage(path: &Path, staged: &Path, file: &mut fs::File) -> io::Result<()> {
    if !starts_with(file, b"\x7fELF")? {
        return Err(invalid_data("The update is not an AppImage"));
    }
    let permissions = fs::metadata(path)?.permissions();
    file.sync_all()?;
    fs::set_permissions(staged, permissions)?;
    fs::rename(staged, path)
}

fn verify_bundle_identity(bundle: &Path) -> io::Result<()> {
    let info = plist::Value::from_file(bundle.join("Contents/Info.plist"))
        .map_err(|_| invalid_data("Cannot read the macOS bundle identity"))?;
    if info
        .as_dictionary()
        .and_then(|p| p.get("CFBundleIdentifier"))
        .and_then(plist::Value::as_string)
        != Some("com.timpan.kubecove.gpui")
    {
        return Err(invalid_data(
            "The bundle identifier is not com.timpan.kubecove.gpui",
        ));
    }
    Ok(())
}

/// Unpacks the staged `.app.tar.gz` beside the installed bundle, then moves the installed bundle
/// aside and the new one into its place. A failed move puts the installed bundle back. The
/// previous bundle is deleted only once the new one is in place, so if putting it back also
/// fails it stays under its hidden name. `rename` is `fs::rename`; tests make it fail.
fn replace_bundle(
    bundle: &Path,
    archive: &mut fs::File,
    rename: impl Fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    if bundle
        .file_name()
        .is_none_or(|name| name != "KubeCove GPUI.app")
    {
        return Err(invalid_data(
            "The installed bundle is not KubeCove GPUI.app",
        ));
    }
    fs::symlink_metadata(bundle)?;
    verify_bundle_identity(bundle)?;
    let staging = sibling(bundle, "update")?;
    let previous = sibling(bundle, "previous")?;
    remove_stale(&staging)?;
    remove_stale(&previous)?;
    fs::create_dir(&staging)?;
    let replaced = archive
        .rewind()
        .and_then(|()| unpack_bundle(archive, &staging))
        .and_then(|unpacked| {
            verify_bundle_identity(&unpacked)?;
            rename(bundle, &previous)?;
            rename(&unpacked, bundle).inspect_err(|_| {
                let _ = rename(&previous, bundle);
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
fn unpack_bundle(archive: impl io::Read, staging: &Path) -> io::Result<PathBuf> {
    tar::Archive::new(flate2::read::GzDecoder::new(archive)).unpack(staging)?;
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
        Ok([bundle])
            if bundle
                .file_name()
                .is_some_and(|name| name == "KubeCove GPUI.app") =>
        {
            Ok(bundle)
        }
        _ => Err(invalid_data("Expected a single KubeCove GPUI.app bundle")),
    }
}

/// Renames the staged installer to `installer` once it holds a Windows executable.
fn save_installer(installer: &Path, staged: &Path, file: &mut fs::File) -> io::Result<()> {
    if !starts_with(file, b"MZ")? {
        return Err(invalid_data("The update is not a Windows installer"));
    }
    file.sync_all()?;
    remove_stale(installer)?;
    fs::rename(staged, installer)
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

/// Failure modes at the bundle swap, written before these tests: the move of the new bundle
/// into place fails, which must leave the installed bundle at its path; putting the installed
/// bundle back fails as well, which must keep it under its hidden name instead of deleting it.
/// Neither may leave the unpacked update behind. `fs::rename` cannot be made to fail at the
/// second move alone from outside, so these tests inject the failure.
#[cfg(test)]
mod tests {
    use super::*;

    const BINARY: &str = "Contents/MacOS/kubecove-gpui";

    /// A test's directory, removed when the test ends, including after a failed
    /// assertion. `tests/native/scratch.rs` is the integration tests' copy.
    struct Root(PathBuf);

    impl Drop for Root {
        fn drop(&mut self) {
            let removed = fs::remove_dir_all(&self.0);
            // A second panic while a failed test unwinds would abort the test binary.
            if !std::thread::panicking() {
                removed.unwrap_or_else(|error| panic!("removing {}: {error}", self.0.display()));
            }
        }
    }

    /// An installed bundle in `<root>/apps`, and an update archive in `<root>`.
    fn installed(name: &str) -> (Root, PathBuf, PathBuf, fs::File) {
        let root = Root(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../.t/update-tests")
                .join(format!("{name}-{}", std::process::id())),
        );
        let _ = fs::remove_dir_all(&root.0);
        let apps = root.0.join("apps");
        let bundle = apps.join("KubeCove GPUI.app");
        fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        let info = include_bytes!("../../../../test-fixtures/side-by-side-info.plist");
        fs::write(bundle.join("Contents/Info.plist"), info).unwrap();
        fs::write(bundle.join(BINARY), b"installed version").unwrap();

        let encoder = flate2::write::GzEncoder::new(
            create_new(&root.0.join("update.tar.gz")).unwrap(),
            flate2::Compression::default(),
        );
        let mut builder = tar::Builder::new(encoder);
        for (path, contents) in [
            (BINARY, b"new version".as_slice()),
            ("Contents/Info.plist", info.as_slice()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o755);
            builder
                .append_data(
                    &mut header,
                    Path::new("KubeCove GPUI.app").join(path),
                    contents,
                )
                .unwrap();
        }
        let archive = builder.into_inner().unwrap().finish().unwrap();
        (root, apps, bundle, archive)
    }

    fn names(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn injected() -> io::Error {
        io::Error::other("injected rename failure")
    }

    #[test]
    fn a_failed_move_into_place_puts_the_installed_bundle_back() {
        let (_root, apps, bundle, mut archive) = installed("swap-fails");
        let previous = sibling(&bundle, "previous").unwrap();
        let result = replace_bundle(&bundle, &mut archive, |from, to| {
            if to == bundle && from != previous {
                Err(injected())
            } else {
                fs::rename(from, to)
            }
        });
        assert!(result.is_err());
        assert_eq!(fs::read(bundle.join(BINARY)).unwrap(), b"installed version");
        assert_eq!(names(&apps), ["KubeCove GPUI.app"]);
    }

    #[test]
    fn a_failed_restore_keeps_the_installed_bundle_under_its_hidden_name() {
        let (_root, apps, bundle, mut archive) = installed("restore-fails");
        let previous = sibling(&bundle, "previous").unwrap();
        let result = replace_bundle(&bundle, &mut archive, |from, to| {
            if to == bundle {
                Err(injected())
            } else {
                fs::rename(from, to)
            }
        });
        assert!(result.is_err());
        assert!(!bundle.exists());
        assert_eq!(
            fs::read(previous.join(BINARY)).unwrap(),
            b"installed version"
        );
        assert_eq!(names(&apps), [".KubeCove GPUI.app.previous"]);
    }
}
