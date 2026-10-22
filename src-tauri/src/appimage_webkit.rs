// AppImage launches prefer the host WebKitGTK stack when it is newer than the bundled copy.
//
// Tauri's AppImage bundler cannot add AppRun hooks (`bundle.linux.appimage.files` only lands
// under `usr/`), and the bundled AppRun prepends the AppImage library dirs to
// `LD_LIBRARY_PATH`. The switch therefore happens here: the binary re-executes itself with the
// host library dir first, so host libraries win and bundled ones only fill gaps. The updater
// signature covers the unchanged AppImage file.

use std::path::Path;

type Version = (u32, u32, u32);

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

// Probe output parsing is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
fn parse_version(text: &str) -> Option<Version> {
    let mut parts = text.trim().split('.').map(|part| part.parse().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    feature = "desktop"
))]
pub fn prefer_newer_system_webkit() {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    // Unset: detect. `bundled`: user opt-out. `probe` and `system`: internal re-exec states.
    const MODE_VAR: &str = "KUBECOVE_APPIMAGE_WEBKIT";
    // glibc `ldconfig -p` prints the 64-bit ABI as `x86-64` or `AArch64` (elf/cache.c).
    #[cfg(target_arch = "x86_64")]
    const ARCH_FLAG: &str = "x86-64";
    #[cfg(target_arch = "aarch64")]
    const ARCH_FLAG: &str = "AArch64";

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

    match std::env::var_os(MODE_VAR) {
        None => {}
        Some(mode) if mode == "probe" => {
            let (major, minor, micro) = loaded_version();
            println!("{major}.{minor}.{micro}");
            std::process::exit(0);
        }
        Some(mode) => {
            // Cleared so an updater relaunch of the AppImage detects again.
            if mode == "system" {
                std::env::remove_var(MODE_VAR);
            }
            return;
        }
    }

    let Some(app_dir) = std::env::var_os("APPDIR").and_then(|dir| std::fs::canonicalize(dir).ok())
    else {
        return;
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if !exe.starts_with(&app_dir) {
        return;
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
    let Some(system_dir) =
        ldconfig_library_path(&cache, "libwebkit2gtk-4.1.so.0", ARCH_FLAG).and_then(Path::parent)
    else {
        return;
    };
    if system_dir.starts_with(&app_dir) {
        return;
    }
    let inherited = std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default();
    let Ok(library_path) = std::env::join_paths(
        std::iter::once(system_dir.to_path_buf()).chain(std::env::split_paths(&inherited)),
    ) else {
        return;
    };

    // The probe loads this binary against the host-first stack. With BIND_NOW, a host stack
    // that cannot satisfy the binary fails here instead of after the re-exec.
    let Some(system_version) = Command::new(&exe)
        .env(MODE_VAR, "probe")
        .env("LD_LIBRARY_PATH", &library_path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| parse_version(&String::from_utf8_lossy(&output.stdout)))
    else {
        return;
    };
    if system_version <= loaded_version() {
        return;
    }

    let mut args = std::env::args_os();
    let mut command = Command::new(&exe);
    if let Some(arg0) = args.next() {
        command.arg0(arg0);
    }
    let error = command
        .args(args)
        .env(MODE_VAR, "system")
        .env("LD_LIBRARY_PATH", &library_path)
        .exec();
    eprintln!("KubeCove: staying on bundled WebKitGTK, relaunch failed: {error}");
}

#[cfg(test)]
mod tests {
    use super::{ldconfig_library_path, parse_version};
    use std::path::Path;

    // Failure modes: matching the unversioned dev symlink, picking another ABI's copy on
    // multilib hosts, rejecting entries with extra ldconfig flags, and accepting malformed
    // probe output as a version.
    const CACHE: &str = "1520 libs found in cache `/etc/ld.so.cache'
\tlibwebkit2gtk-4.1.so.0 (libc6) => /usr/lib32/libwebkit2gtk-4.1.so.0
\tlibwebkit2gtk-4.1.so.0 (libc6,x86-64, OS ABI: Linux 3.2.0) => /usr/lib/x86_64-linux-gnu/libwebkit2gtk-4.1.so.0
\tlibwebkit2gtk-4.1.so (libc6,x86-64) => /opt/dev/libwebkit2gtk-4.1.so
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

    #[test]
    fn probe_output_must_be_exactly_three_numbers() {
        assert_eq!(parse_version("2.54.1\n"), Some((2, 54, 1)));
        assert_eq!(parse_version("2.54"), None);
        assert_eq!(parse_version("2.54.1.0"), None);
        assert_eq!(parse_version("Gtk-WARNING\n2.54.1"), None);
        assert_eq!(parse_version(""), None);
    }
}
