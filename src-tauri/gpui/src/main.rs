use gpui_kit::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::{
    Inspector,
    assets::AppAssets,
    backend::{DesktopBackend, NativeBackend, settings_integrations::config_directory},
};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
