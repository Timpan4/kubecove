//! CPU frame measurements. Does not claim GPU presentation or a release-build rate.
#[path = "../common/mod.rs"]
mod common;
use super::*;
use std::{path::Path, time::Instant};

#[gpui_kit::test]
#[ignore = "requires the exported 10,000-row benchmark fixture"]
fn watch_burst_frame_time(cx: &mut TestAppContext) {
    let path = std::env::var_os("KUBECOVE_WATCH_BURST_FIXTURE")
        .expect("export-watch-burst.ts fixture required");
    let backend = Arc::new(common::Fixture::load(Path::new(&path)));
    cx.update(gpui_kit::init);
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(1440.), px(900.)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    Inspector::new(backend.clone(), window, cx).with_saved_workspace(
                        "Parity mock workspace",
                        "mock-dev",
                        cx,
                    )
                })
            },
        )
        .unwrap()
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-resources", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    let baseline = Instant::now();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    let baseline_ms = baseline.elapsed().as_secs_f64() * 1000.;
    backend.replay_live("burst");
    let start = Instant::now();
    cx.run_until_parked();
    let drain_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    let frame_ms = start.elapsed().as_secs_f64() * 1000.;
    println!(
        "watch-burst rows=10000 messages=10000 baseline_frame_ms={baseline_ms:.3} drain_ms={drain_ms:.3} burst_frame_ms={frame_ms:.3} budget_ms=8.33"
    );
    cx.update_window(handle, |_, window, cx| {
        window.click("search", cx);
        window.input("checkout-6-deployment-6", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window
                .find("resource:Deployment:namespace-6:checkout-6-deployment-6")
                .description(),
            Some("Degraded")
        );
    })
    .unwrap();
    #[cfg(feature = "frame-profile")]
    println!(
        "watch phases {}",
        serde_json::to_string(&kubecove_gpui::frame_profile::take()).unwrap()
    );
}
