use gpui_kit::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::{Inspector, assets::AppAssets, backend::NativeBackend};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = Arc::new(NativeBackend::new()?);
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
