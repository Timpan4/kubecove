use gpui_kit::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::{
    Inspector,
    assets::AppAssets,
    backend::{DesktopBackend, NativeBackend, settings_integrations::config_directory},
};
use std::sync::Arc;

/// The packager `identifier` in `Cargo.toml`, which names the installed
/// `.desktop` file. A Wayland compositor and an X11 window manager match the
/// window to that file, and so to its icon, by this ID.
const APP_ID: &str = "com.timpan.kubecove.gpui";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    cap_malloc_arenas();
    // reqwest is built with `rustls-no-provider`, so a TLS client such as the update check
    // panics unless a provider is installed first. Only the first installation takes effect.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let directory = config_directory()?;
    let legacy_sessions = directory
        .with_file_name("kubecove-gpui")
        .join("live-sessions.json");
    let local = NativeBackend::with_settings_directory(directory)?
        .with_legacy_session_file(legacy_sessions);
    let backend = Arc::new(DesktopBackend::new(local));
    gpui_kit::application()
        .with_assets(AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            if let Err(error) = kubecove_gpui::init(cx) {
                eprintln!("Could not load KubeCove fonts: {error}");
                cx.quit();
                return;
            }
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            if let Err(error) = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus: false,
                    app_id: Some(APP_ID.into()),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    window.set_window_title("KubeCove");
                    cx.new(|cx| Inspector::new(backend, window, cx))
                },
            ) {
                eprintln!("Could not open KubeCove: {error}");
                cx.quit();
            }
        });
    Ok(())
}

/// glibc gives allocating threads their own arenas, up to eight per core, and freed per-frame
/// buffers stay resident in each. Two arenas measured 691 MiB PSS at 10,000 Pods against
/// 799 MiB with the default (`MALLOC_ARENA_MAX=2`, GPUI cost map item 8).
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn cap_malloc_arenas() {
    /// `M_ARENA_MAX` in glibc's `malloc.h`.
    const M_ARENA_MAX: std::ffi::c_int = -8;
    unsafe extern "C" {
        fn mallopt(param: std::ffi::c_int, value: std::ffi::c_int) -> std::ffi::c_int;
    }
    // SAFETY: mallopt only sets an allocator tunable, and it runs before any other thread starts.
    unsafe { mallopt(M_ARENA_MAX, 2) };
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn cap_malloc_arenas() {}
