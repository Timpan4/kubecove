//! Production-mode CPU frame timing in headless SDK benchmark windows.
//! Each interaction records every requested frame, its state/update work plus
//! render, the input-to-settled wall latency, and the number of draws to settle.
//! The frame context never enters GpuiMode::Test. GPU readback is a separate
//! replay and does not measure display presentation. See FRAME_TIMING.md.

#[path = "frames/cases.rs"]
mod cases;
mod common;
#[path = "frames/context.rs"]
mod context;
#[path = "frames/fixture.rs"]
mod fixture;
#[path = "frames/measure.rs"]
mod measure;
#[path = "frames/platform.rs"]
mod platform;

use cases::{Case, all_cases};
use common::Fixture;
use context::FrameContext;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::Inspector;
use measure::{gpu_floor, measure};
use serde_json::{Value, json};
use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

/// The window the visual and parity drivers use.
const WIDTH: f32 = 1440.;
const HEIGHT: f32 = 900.;

pub struct Driver {
    #[cfg(feature = "frame-profile")]
    inspector: gpui_kit::Entity<Inspector>,
    pub backend: Arc<Fixture>,
    pub terminal_burst: String,
    pub cx: FrameContext,
    pub handle: AnyWindowHandle,
    pub detail_page: usize,
    pub table_page: usize,
}

fn window_options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Default::default(),
            size: size(px(WIDTH), px(HEIGHT)),
        })),
        focus: true,
        show: true,
        ..Default::default()
    }
}

impl Driver {
    fn open(gpu: bool, detail_page: usize, fixture_name: Option<&str>) -> Self {
        // Each CPU and renderer replay starts from the same untouched fixture.
        let report =
            PathBuf::from(std::env::var_os("KUBECOVE_FRAMES_REPORT").expect("report path"));
        let backend = Arc::new(Fixture::load(
            &report
                .parent()
                .expect("report directory")
                .join(fixture_name.unwrap_or("fixture-scaled.json")),
        ));
        let mut cx = FrameContext::new(gpu);
        let executor = cx.update(|cx| cx.background_executor().clone());
        backend
            .off_thread_reads
            .set((Arc::downgrade(&backend), executor))
            .unwrap_or_else(|_| panic!("a new fixture has no executor"));
        let (handle, _inspector) = cx
            .update(|cx| {
                gpui_kit::open_window(window_options(), cx, |window, cx| {
                    cx.new(|cx| {
                        Inspector::new(backend.clone(), window, cx).with_saved_workspace(
                            "Parity mock workspace",
                            "mock-dev",
                            cx,
                        )
                    })
                })
            })
            .unwrap();
        cx.update_window(handle, |_, window, _| window.activate_window())
            .unwrap();
        cx.settle();
        Self {
            #[cfg(feature = "frame-profile")]
            inspector: _inspector,
            backend,
            terminal_burst: std::fs::read_to_string(
                report.parent().unwrap().join("terminal-burst.txt"),
            )
            .expect("terminal burst fixture"),
            cx,
            handle,
            detail_page,
            table_page: 0,
        }
    }

    /// An unmeasured step: draw, act on that frame, then settle.
    pub fn act(&mut self, action: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App)) {
        self.cx
            .update_window(self.handle, |_, window, cx| {
                window.render_frame(cx);
                action(window, cx);
            })
            .unwrap();
        self.cx.tick();
        self.draw();
    }

    pub fn draw(&mut self) {
        self.cx
            .update_window(self.handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        self.cx.tick();
    }

    pub fn read<R>(&mut self, read: impl FnOnce(&mut gpui_kit::Window) -> R) -> R {
        self.cx
            .update_window(self.handle, |_, window, _| read(window))
            .unwrap()
    }

    /// A hash of the last scene's quads and the case's probe, to check that a
    /// step changed what is drawn.
    fn scene(&mut self, case: &Case) -> u64 {
        self.read(|window| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            for quad in window.painted_quads() {
                format!("{:?}{:?}", quad.bounds, quad.background).hash(&mut hasher);
            }
            if let Some(probe) = case.probe {
                probe(window).hash(&mut hasher);
            }
            hasher.finish()
        })
    }
}

fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn env_number(key: &str) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("{key} must be a number"))
}

fn main() {
    let (Some(fixture), Some(report)) = (
        std::env::var_os("KUBECOVE_FRAMES_FIXTURE"),
        std::env::var_os("KUBECOVE_FRAMES_REPORT"),
    ) else {
        println!("frames: skipped; run `bun scripts/gpui-frames.ts`");
        return;
    };
    gpui_kit::profiler::set_trace_enabled(true);
    let warmup = env_number("KUBECOVE_FRAMES_WARMUP");
    let frames = env_number("KUBECOVE_FRAMES_COUNT");
    let gpu = std::env::var("KUBECOVE_FRAMES_GPU").is_ok_and(|value| value == "1");
    let selected: Option<Vec<String>> = std::env::var("KUBECOVE_FRAMES_CASES")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| value.split(',').map(str::to_owned).collect());
    let mut exported: Value =
        serde_json::from_slice(&std::fs::read(&fixture).expect("fixture readable")).unwrap();
    let mut sizes = fixture::scale(&mut exported);
    let (pods, source) = fixture::FAN_OUT_PODS;
    sizes.insert(
        "fan-out map pods".to_owned(),
        serde_json::json!({"count": pods, "source": source}),
    );
    let detail_page = fixture::detail_page(&exported);
    let report = PathBuf::from(report);
    let directory = report.parent().unwrap_or(Path::new(".")).to_owned();
    std::fs::create_dir_all(&directory).unwrap();
    // The scaled fixture stays beside the report as the measured input.
    let scaled = directory.join("fixture-scaled.json");
    std::fs::write(&scaled, serde_json::to_vec(&exported).unwrap()).unwrap();
    std::fs::write(
        directory.join("fixture-fan-out.json"),
        serde_json::to_vec(&fixture::fan_out(&exported)).unwrap(),
    )
    .unwrap();
    let burst = exported["detailTabs"]["logs"]["pod"]
        .as_array()
        .expect("pod log fixture")
        .iter()
        .filter(|batch| batch["type"] == "logLines")
        .flat_map(|batch| batch["lines"].as_array().expect("log lines"))
        .map(|line| line.as_str().expect("log text"))
        .collect::<Vec<_>>()
        .join("\r\n");
    std::fs::write(directory.join("terminal-burst.txt"), format!("{burst}\r\n")).unwrap();
    let watch_path = directory.join("fixture-watch-burst.json");
    let mut watch: Value =
        serde_json::from_slice(&std::fs::read(&watch_path).expect("watch workload")).unwrap();
    watch["contextResources"] = watch["resources"].clone();
    assert_eq!(watch["resources"].as_array().unwrap().len(), 10_000);
    for &count in cases::multi_context::COUNTS {
        std::fs::write(
            directory.join(format!("fixture-contexts-{count}.json")),
            serde_json::to_vec(&fixture::contexts(&watch, count)).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(watch_path, serde_json::to_vec(&watch).unwrap()).unwrap();
    let mut results = Vec::new();
    if let Some(ids) = &selected {
        for id in ids {
            assert!(
                all_cases().any(|case| case.id == id),
                "unknown frame case: {id}"
            );
        }
    }
    for case in all_cases() {
        if selected
            .as_ref()
            .is_some_and(|ids| !ids.iter().any(|id| id == case.id))
        {
            continue;
        }
        let started = Instant::now();
        let timings = measure(case, warmup, frames, gpu, detail_page);
        if case.step.is_some() {
            assert_eq!(
                timings.changed, frames,
                "{} included idle or ineffective input",
                case.id
            );
        }
        println!(
            "frames {}: {} samples, {} changed, {:.1}s",
            case.id,
            timings.draw.len(),
            timings.changed,
            started.elapsed().as_secs_f32()
        );
        results.push(json!({
            "id": case.id,
            "kind": if case.step.is_some() { "interaction" } else { "view" },
            "description": case.description,
            "step": timings.step,
            "cpuFrames": timings.cpu_frames,
            "scheduler": timings.scheduler,
            "gpuCpuFrames": timings.gpu_cpu_frames,
            "gpuStep": timings.gpu_step,
            "gpuScheduler": timings.gpu_scheduler,
            "draw": timings.draw,
            "gpu": timings.gpu,
            "gpuSize": timings.gpu_size,
            "changed": timings.changed,
            "coldDraw": timings.cold_draw,
            "profile": kubecove_gpui::frame_profile::take(),
        }));
        std::fs::write(
            directory.join("partial.json"),
            serde_json::to_vec(&results).unwrap(),
        )
        .unwrap();
    }
    let floor = if gpu {
        gpu_floor(warmup, frames)
    } else {
        vec![]
    };
    std::fs::write(
        &report,
        serde_json::to_vec(&json!({
            // 3: time blocked on background work during a drain is `wait`, not frame CPU.
            "measurementVersion": 3,
            "mode": "Production",
            "build": {
                "manifestDirectory": env!("CARGO_MANIFEST_DIR"),
                "profiling": cfg!(feature = "frame-profile"),
                "debugAssertions": cfg!(debug_assertions),
                "targetDirectory": option_env!("CARGO_TARGET_DIR"),
                "buildDirectory": option_env!("CARGO_BUILD_BUILD_DIR"),
            },
            "window": {"width": WIDTH, "height": HEIGHT},
            "warmup": warmup,
            "frames": frames,
            "sizes": sizes,
            "gpuFloor": floor,
            "cases": results,
        }))
        .unwrap(),
    )
    .unwrap();
    println!("frames: report -> {}", report.display());
}
