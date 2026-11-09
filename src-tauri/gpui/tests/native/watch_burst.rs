//! Measurements on the exported 10,000-row watch-burst workload: CPU frame time
//! of a watch burst and process RSS. Does not claim GPU presentation or a
//! release-build rate. `KUBECOVE_WATCH_BURST_CONTEXTS` (default 1) copies the
//! workload into that many workspace contexts (`frames/fixture.rs`).
#[path = "../common/mod.rs"]
mod common;
// The frames harness uses the rest of its fixture builder.
#[allow(dead_code)]
#[path = "../frames/fixture.rs"]
mod frames_fixture;
use super::*;
use gpui_kit::{AnyWindowHandle, ElementId};
use std::{path::Path, time::Instant};

fn context_count() -> usize {
    std::env::var("KUBECOVE_WATCH_BURST_CONTEXTS")
        .map(|count| count.parse().expect("context count is a number"))
        .unwrap_or(1)
}

fn named(id: String) -> ElementId {
    ElementId::Name(id.into())
}

fn act(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    action: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App),
) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        action(window, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn click_label(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, label: &str) {
    let target = window
        .within("launcher-body")
        .find_by_label(label)
        .path()
        .last()
        .expect("labeled element has an ID")
        .clone();
    window.click(target, cx);
}

/// The workload in `count` contexts, written beside the exported fixture.
fn load_backend(count: usize) -> Arc<common::Fixture> {
    let path = std::env::var_os("KUBECOVE_WATCH_BURST_FIXTURE")
        .expect("export-watch-burst.ts fixture required");
    let path = Path::new(&path);
    let watch: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("fixture readable")).unwrap();
    let written = path.with_extension(format!("contexts-{count}.json"));
    std::fs::write(
        &written,
        serde_json::to_vec(&frames_fixture::contexts(&watch, count)).unwrap(),
    )
    .unwrap();
    Arc::new(common::Fixture::load(&written))
}

/// Opens the parity workspace with `count` contexts and shows Resources.
fn open_resources(
    cx: &mut TestAppContext,
    backend: Arc<common::Fixture>,
    count: usize,
) -> AnyWindowHandle {
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
                        frames_fixture::context_name(0),
                        cx,
                    )
                })
            },
        )
        .unwrap()
    });
    cx.run_until_parked();
    if count > 1 {
        act(cx, handle, |window, cx| window.click("header-workspaces", cx));
        act(cx, handle, |window, cx| {
            click_label(window, cx, "Edit Parity mock workspace")
        });
        for rank in 1..count {
            act(cx, handle, |window, cx| {
                let target = format!("workspace-group:{}", frames_fixture::context_name(rank));
                window.click(named(target), cx);
            });
        }
        act(cx, handle, |window, cx| window.click("workspace-create", cx));
        act(cx, handle, |window, cx| {
            click_label(window, cx, "Open Parity mock workspace")
        });
    }
    act(cx, handle, |window, cx| {
        window.click("open-resources", cx);
        window.render_frame(cx);
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let summary = window.find("resource-table-summary").label().unwrap_or_default().to_owned();
        assert!(
            summary.starts_with(&format!("{} resources", count * 10_000)),
            "{count} contexts must list {} rows: {summary}",
            count * 10_000
        );
    })
    .unwrap();
    handle
}

#[gpui_kit::test]
#[ignore = "requires the exported 10,000-row benchmark fixture"]
fn watch_burst_frame_time(cx: &mut TestAppContext) {
    let count = context_count();
    let backend = load_backend(count);
    let handle = open_resources(cx, backend.clone(), count);
    let baseline = Instant::now();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    let baseline_ms = baseline.elapsed().as_secs_f64() * 1000.;
    #[cfg(feature = "frame-profile")]
    println!(
        "load phases {}",
        serde_json::to_string(&kubecove_gpui::frame_profile::take()).unwrap()
    );
    // The burst reaches the first context's watches only.
    backend.replay_burst_in(&frames_fixture::context_name(0));
    let start = Instant::now();
    cx.run_until_parked();
    let drain_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    let frame_ms = start.elapsed().as_secs_f64() * 1000.;
    println!(
        "watch-burst contexts={count} rows={} messages=10000 baseline_frame_ms={baseline_ms:.3} drain_ms={drain_ms:.3} burst_frame_ms={frame_ms:.3} budget_ms=8.33",
        count * 10_000
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
                .find("resource:Deployment:namespace-6:checkout-6-deployment-6:mock-dev")
                .description(),
            Some("Degraded")
        );
        // Other contexts were not notified, so their rows keep their health.
        for rank in 1..count {
            let id = format!(
                "resource:Deployment:namespace-6:checkout-6-deployment-6:{}",
                frames_fixture::context_name(rank)
            );
            assert_ne!(window.find(named(id)).description(), Some("Degraded"));
        }
    })
    .unwrap();
    #[cfg(feature = "frame-profile")]
    println!(
        "watch phases {}",
        serde_json::to_string(&kubecove_gpui::frame_profile::take()).unwrap()
    );
}

/// `VmRSS` and `VmHWM` of this process in KiB, where `/proc` exists.
fn memory_kib() -> Option<(u64, u64)> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
    };
    Some((field("VmRSS:")?, field("VmHWM:")?))
}

fn report_memory(phase: &str, count: usize) {
    match memory_kib() {
        Some((rss, peak)) => println!(
            "multi-context-rss contexts={count} phase={phase} rss_mib={:.1} hwm_mib={:.1}",
            rss as f64 / 1024.,
            peak as f64 / 1024.
        ),
        None => println!("multi-context-rss contexts={count} phase={phase} unavailable"),
    }
}

#[gpui_kit::test]
#[ignore = "requires the exported 10,000-row benchmark fixture"]
fn multi_context_rss(cx: &mut TestAppContext) {
    let count = context_count();
    report_memory("start", count);
    let backend = load_backend(count);
    report_memory("fixture-loaded", count);
    let handle = open_resources(cx, backend, count);
    report_memory("initial-load", count);
    // Refresh lives in the Live updates menu, as in the web top bar.
    act(cx, handle, |window, cx| window.click("live-status", cx));
    act(cx, handle, |window, cx| window.click("refresh", cx));
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    report_memory("after-refresh", count);
}
