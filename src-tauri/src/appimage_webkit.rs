// AppImage launches prefer the host WebKitGTK stack when it is newer than the bundled copy.
//
// Tauri's AppImage bundler cannot add AppRun hooks (`bundle.linux.appimage.files` only lands
// under `usr/`), and the bundled AppRun prepends the AppImage library dirs to
// `LD_LIBRARY_PATH`. The switch therefore happens here: the binary re-executes itself with the
// host library dir first, so host libraries win and bundled ones only fill gaps. The updater
// signature covers the unchanged AppImage file.

use std::ffi::{OsStr, OsString};
use std::path::Path;

type Version = (u32, u32, u32);

// The re-exec swaps the whole stack only when these share one host directory; otherwise host
// WebKit would resolve GTK or GLib from the AppImage.
const HOST_STACK: [&str; 3] = [
    "libwebkit2gtk-4.1.so.0",
    "libgtk-3.so.0",
    "libglib-2.0.so.0",
];

// ldconfig cache parsing is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
fn ldconfig_library_path<'a>(cache: &'a str, soname: &str, arch_flag: &str) -> Option<&'a Path> {
    cache.lines().find_map(|line| {
        let (entry, path) = line.trim_start().split_once(" => ")?;
        let (name, flags) = entry.split_once(" (")?;
        let flags = flags.strip_suffix(')')?;
        (name == soname && flags.split(',').any(|flag| flag.trim() == arch_flag))
            .then(|| Path::new(path.trim()))
    })
}

// Compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
fn host_stack_dir<'a>(cache: &'a str, arch_flag: &str) -> Option<&'a Path> {
    let mut dirs = HOST_STACK
        .iter()
        .map(|soname| ldconfig_library_path(cache, soname, arch_flag).and_then(Path::parent));
    let first = dirs.next()??;
    dirs.all(|dir| dir == Some(first)).then_some(first)
}

// Probe output parsing is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
fn parse_version(text: &str) -> Option<Version> {
    let mut parts = text.trim().split('.').map(|part| part.parse().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

// Compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
fn is_under(value: &OsStr, app_dirs: &[&Path]) -> bool {
    app_dirs.iter().any(|dir| Path::new(value).starts_with(dir))
}

// Removes AppImage entries from a path list such as `XDG_DATA_DIRS`. Compiled on every
// platform so shared tests can cover it.
#[allow(dead_code)]
fn without_app_dirs(list: &OsStr, app_dirs: &[&Path]) -> Option<OsString> {
    let kept: Vec<_> = std::env::split_paths(list)
        .filter(|entry| !entry.as_os_str().is_empty() && !is_under(entry.as_os_str(), app_dirs))
        .collect();
    std::env::join_paths(kept).ok()
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    feature = "desktop"
))]
pub fn prefer_newer_system_webkit() {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    // `bundled`: user opt-out. `probe` and `system`: internal re-exec states. Other values are
    // ignored so a mistyped setting cannot silently disable detection.
    const MODE_VAR: &str = "KUBECOVE_APPIMAGE_WEBKIT";
    // glibc `ldconfig -p` prints the 64-bit ABI as `x86-64` or `AArch64` (elf/cache.c).
    #[cfg(target_arch = "x86_64")]
    const ARCH_FLAG: &str = "x86-64";
    #[cfg(target_arch = "aarch64")]
    const ARCH_FLAG: &str = "AArch64";
    // linuxdeploy-plugin-gtk's AppRun hook points these at bundled GTK, GIO and GSettings data.
    // Host GTK and GLib must load their own modules and schemas instead.
    const BUNDLED_GTK_VARS: [&str; 8] = [
        "GTK_DATA_PREFIX",
        "GSETTINGS_SCHEMA_DIR",
        "GI_TYPELIB_PATH",
        "GIO_MODULE_DIR",
        "GTK_EXE_PREFIX",
        "GTK_PATH",
        "GTK_IM_MODULE_FILE",
        "GDK_PIXBUF_MODULE_FILE",
    ];

    fn loaded_version() -> Version {
        // SAFETY: argument-free WebKitGTK getters from the already linked library.
        unsafe {
            (
                webkit2gtk::ffi::webkit_get_major_version(),
                webkit2gtk::ffi::webkit_get_minor_version(),
                webkit2gtk::ffi::webkit_get_micro_version(),
            )
        }
    }

    let mode = std::env::var_os(MODE_VAR);
    match mode.as_deref().and_then(OsStr::to_str) {
        Some("bundled") => return,
        Some("system") => {
            // Cleared so an updater relaunch of the AppImage detects again.
            std::env::remove_var(MODE_VAR);
            return;
        }
        _ => {}
    }

    let Some(raw_app_dir) = std::env::var_os("APPDIR") else {
        return;
    };
    let Ok(app_dir) = std::fs::canonicalize(&raw_app_dir) else {
        return;
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if !exe.starts_with(&app_dir) {
        return;
    }

    if let Some(mode) = &mode {
        if mode == "probe" {
            let (major, minor, micro) = loaded_version();
            println!("{major}.{minor}.{micro}");
            std::process::exit(0);
        }
        eprintln!(
            "KubeCove: ignoring {MODE_VAR}={}; set it to `bundled` to keep the bundled WebKitGTK",
            mode.display()
        );
    }

    // Debian-family user PATHs may omit the sbin dirs.
    let Some(cache) = ["ldconfig", "/sbin/ldconfig", "/usr/sbin/ldconfig"]
        .into_iter()
        .find_map(|ldconfig| {
            Command::new(ldconfig)
                .arg("-p")
                .stderr(Stdio::null())
                .output()
                .ok()
                .filter(|output| output.status.success())
        })
    else {
        return;
    };
    let cache = String::from_utf8_lossy(&cache.stdout);
    let Some(system_dir) = host_stack_dir(&cache, ARCH_FLAG) else {
        return;
    };
    let app_dirs = [Path::new(&raw_app_dir), app_dir.as_path()];
    if is_under(system_dir.as_os_str(), &app_dirs) {
        return;
    }
    let inherited = std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default();
    let Ok(library_path) = std::env::join_paths(
        std::iter::once(system_dir.to_path_buf()).chain(std::env::split_paths(&inherited)),
    ) else {
        return;
    };

    // The probe loads this binary against the host-first stack. LD_BIND_NOW resolves every
    // library's symbols at load, so an incompatible host stack fails here instead of after the
    // re-exec. The probe does not initialize GTK or start a web process.
    let Some(system_version) = Command::new(&exe)
        .env(MODE_VAR, "probe")
        .env("LD_LIBRARY_PATH", &library_path)
        .env("LD_BIND_NOW", "1")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| parse_version(&String::from_utf8_lossy(&output.stdout)))
    else {
        return;
    };
    let bundled_version = loaded_version();
    if system_version <= bundled_version {
        return;
    }

    let mut args = std::env::args_os();
    let mut command = Command::new(&exe);
    if let Some(arg0) = args.next() {
        command.arg0(arg0);
    }
    for var in BUNDLED_GTK_VARS {
        if std::env::var_os(var).is_some_and(|value| is_under(&value, &app_dirs)) {
            command.env_remove(var);
        }
    }
    if let Some(data_dirs) =
        std::env::var_os("XDG_DATA_DIRS").and_then(|dirs| without_app_dirs(&dirs, &app_dirs))
    {
        command.env("XDG_DATA_DIRS", data_dirs);
    }
    let (major, minor, micro) = system_version;
    let (bundled_major, bundled_minor, bundled_micro) = bundled_version;
    eprintln!(
        "KubeCove: using host WebKitGTK {major}.{minor}.{micro} from {} instead of bundled \
         {bundled_major}.{bundled_minor}.{bundled_micro}; set {MODE_VAR}=bundled to keep the bundled stack",
        system_dir.display()
    );
    let error = command
        .args(args)
        .env(MODE_VAR, "system")
        .env("LD_LIBRARY_PATH", &library_path)
        .exec();
    eprintln!("KubeCove: staying on bundled WebKitGTK, relaunch failed: {error}");
}

#[cfg(test)]
mod tests {
    use super::{host_stack_dir, ldconfig_library_path, parse_version, without_app_dirs};
    use std::ffi::OsStr;
    use std::path::Path;

    // Failure modes: matching the unversioned dev symlink, picking another ABI's copy on
    // multilib hosts, rejecting entries with extra ldconfig flags, and accepting malformed
    // probe output as a version.
    const CACHE: &str = "1520 libs found in cache `/etc/ld.so.cache'
\tlibwebkit2gtk-4.1.so.0 (libc6) => /usr/lib32/libwebkit2gtk-4.1.so.0
\tlibwebkit2gtk-4.1.so.0 (libc6,x86-64, OS ABI: Linux 3.2.0) => /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0
\tlibwebkit2gtk-4.1.so (libc6,x86-64) => /opt/dev/libwebkit2gtk-4.1.so
\tlibgtk-3.so.0 (libc6,x86-64) => /usr/lib/x86_64-linux-gnu/libgtk-3.so.0
\tlibglib-2.0.so.0 (libc6) => /usr/lib32/libglib-2.0.so.0
\tlibglib-2.0.so.0 (libc6,x86-64) => /usr/lib/x86_64-linux-gnu/libglib-2.0.so.0
";

    #[test]
    fn ldconfig_picks_versioned_soname_for_matching_abi() {
        assert_eq!(
            ldconfig_library_path(CACHE, "libwebkit2gtk-4.1.so.0", "x86-64"),
            Some(Path::new(
                "/usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0"
            ))
        );
        assert_eq!(
            ldconfig_library_path(CACHE, "libwebkit2gtk-4.1.so.0", "AArch64"),
            None
        );
        assert_eq!(
            ldconfig_library_path(CACHE, "libjavascriptcoregtk-4.1.so.0", "x86-64"),
            None
        );
    }

    // Failure modes: switching when host WebKit sits apart from host GTK or GLib (for example
    // in /usr/local or a glibc-hwcaps subdir), which would mix host WebKit with bundled GTK.
    #[test]
    fn host_stack_requires_webkit_gtk_and_glib_in_one_directory() {
        assert_eq!(
            host_stack_dir(CACHE, "x86-64"),
            Some(Path::new("/usr/lib/x86_64-linux-gnu"))
        );
        let split = CACHE.replace(
            "=> /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0",
            "=> /usr/local/lib/libwebkit2gtk-4.1.so.0",
        );
        assert_eq!(host_stack_dir(&split, "x86-64"), None);
        let missing_gtk = CACHE.replace("libgtk-3.so.0 (libc6,x86-64)", "libgtk-3.so.0 (libc6)");
        assert_eq!(host_stack_dir(&missing_gtk, "x86-64"), None);
    }

    #[test]
    fn probe_output_must_be_exactly_three_numbers() {
        assert_eq!(parse_version("2.54.1\n"), Some((2, 54, 1)));
        assert_eq!(parse_version("2.54"), None);
        assert_eq!(parse_version("2.54.1.0"), None);
        assert_eq!(parse_version("Gtk-WARNING\n2.54.1"), None);
        assert_eq!(parse_version(""), None);
    }

    // Failure modes: keeping the hook's `$APPDIR//usr/share` entry (double slash), or dropping
    // host entries that merely share a name prefix with the mount point.
    #[test]
    fn data_dirs_drop_only_appimage_entries() {
        let app_dir = Path::new("/tmp/.mount_KubeCo");
        let dirs = OsStr::new(
            "/tmp/.mount_KubeCo//usr/share:/usr/share:/tmp/.mount_KubeCoX/share:/usr/local/share",
        );
        assert_eq!(
            without_app_dirs(dirs, &[app_dir]).as_deref(),
            Some(OsStr::new(
                "/usr/share:/tmp/.mount_KubeCoX/share:/usr/local/share"
            ))
        );
    }
}
