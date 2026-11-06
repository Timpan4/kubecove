//! Fixed warm-up and measurement protocol shared by the CPU and GPU replays.

use super::context::CpuFrame;
use super::{Case, Driver, FrameContext, nanos, window_options};
#[cfg(feature = "frame-profile")]
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Pixels, Point};
use std::{path::PathBuf, time::Instant};

pub(super) struct Timings {
    pub step: Vec<u64>,
    pub draw: Vec<u64>,
    pub cpu_frames: Vec<Vec<CpuFrame>>,
    pub scheduler: Vec<Option<[u64; 2]>>,
    pub gpu_cpu_frames: Vec<Vec<CpuFrame>>,
    pub gpu_step: Vec<u64>,
    pub gpu_scheduler: Vec<Option<[u64; 2]>>,
    pub gpu: Vec<u64>,
    pub gpu_size: Option<(u32, u32)>,
    pub changed: usize,
    pub cold_draw: u64,
}

fn aim(driver: &mut Driver, case: &Case, index: usize) -> Point<Pixels> {
    if let Some(prepare) = case.prepare {
        prepare(driver, index);
    }
    driver.read(|window| (case.aim)(window, index))
}

fn frame(
    driver: &mut Driver,
    case: &Case,
    index: usize,
    at: Point<Pixels>,
) -> (u64, Vec<CpuFrame>, Option<[u64; 2]>) {
    driver.cx.begin_sample();
    driver
        .cx
        .update_window(driver.handle, |_, window, cx| {
            if let Some(replay) = case.replay {
                replay(&driver.backend, &driver.terminal_burst, index);
            }
            if let Some(step) = case.step {
                step(window, cx, index, at);
            } else {
                window.refresh();
            }
        })
        .unwrap();
    if case.step.is_some() {
        driver.cx.settle();
    } else {
        driver.cx.tick();
    }
    let (settled, frames, scheduler) = driver.cx.finish_sample();
    assert!(
        frames.iter().any(|frame| !frame.presentation_only),
        "{} input {} produced no draw",
        case.id,
        index
    );
    (settled, frames, scheduler)
}

pub(super) fn measure(
    case: &Case,
    warmup: usize,
    frames: usize,
    gpu: bool,
    detail_page: usize,
) -> Timings {
    kubecove_gpui::frame_profile::set_recording(false);
    let mut driver = Driver::open(false, detail_page, case.fixture);
    (case.setup)(&mut driver);
    #[cfg(feature = "frame-profile")]
    if matches!(case.id, "events-scroll" | "timeline-scroll") {
        scroll_profile(&mut driver, "before");
    }
    let mut timings = Timings {
        step: Vec::with_capacity(frames),
        draw: Vec::with_capacity(frames),
        cpu_frames: Vec::with_capacity(frames),
        scheduler: Vec::with_capacity(frames),
        gpu_cpu_frames: Vec::new(),
        gpu_step: Vec::new(),
        gpu_scheduler: Vec::new(),
        gpu: Vec::new(),
        gpu_size: None,
        changed: 0,
        cold_draw: 0,
    };
    for index in 0..warmup {
        let at = aim(&mut driver, case, index);
        let (_, draws, _) = frame(&mut driver, case, index, at);
        if index == 0 {
            timings.cold_draw = draws[0].draw;
        }
    }
    #[cfg(feature = "frame-profile")]
    if matches!(case.id, "events-scroll" | "timeline-scroll") {
        scroll_profile(&mut driver, "after-warmup");
    }
    kubecove_gpui::frame_profile::take();
    for index in warmup..warmup + frames {
        let at = aim(&mut driver, case, index);
        let before = driver.scene(case);
        kubecove_gpui::frame_profile::set_recording(true);
        let (settled, draws, scheduler) = frame(&mut driver, case, index, at);
        kubecove_gpui::frame_profile::set_recording(false);
        if case.step.is_some() {
            timings.step.push(settled);
        }
        timings
            .draw
            .push(draws.iter().map(|frame| frame.draw).max().unwrap());
        timings.cpu_frames.push(draws);
        timings.scheduler.push(scheduler);
        timings.changed += usize::from(driver.scene(case) != before);
    }
    drop(driver);
    if gpu {
        let mut driver = Driver::open(true, detail_page, case.fixture);
        (case.setup)(&mut driver);
        for index in 0..warmup {
            let at = aim(&mut driver, case, index);
            frame(&mut driver, case, index, at);
        }
        timings.gpu.reserve(frames);
        timings.gpu_cpu_frames.reserve(frames);
        timings.gpu_step.reserve(frames);
        timings.gpu_scheduler.reserve(frames);
        for index in warmup..warmup + frames {
            let at = aim(&mut driver, case, index);
            let before = driver.scene(case);
            kubecove_gpui::frame_profile::set_recording(true);
            let (settled, gpu_frames, scheduler) = frame(&mut driver, case, index, at);
            kubecove_gpui::frame_profile::set_recording(false);
            if case.step.is_some() {
                timings.gpu_step.push(settled);
                assert_ne!(
                    driver.scene(case),
                    before,
                    "{} renderer replay included idle or ineffective input",
                    case.id
                );
            }
            timings.gpu_cpu_frames.push(gpu_frames);
            timings.gpu_scheduler.push(scheduler);
            let start = Instant::now();
            let image = driver
                .cx
                .update_window(driver.handle, |_, window, _| window.render_to_image())
                .unwrap();
            let elapsed = start.elapsed();
            if let Err(error) = image {
                eprintln!("{} offscreen rendering failed: {error:#}", case.id);
                timings.gpu.clear();
                timings.gpu_cpu_frames.clear();
                timings.gpu_step.clear();
                timings.gpu_scheduler.clear();
                break;
            }
            timings.gpu.push(nanos(elapsed));
        }
        if !timings.gpu.is_empty() {
            let report = PathBuf::from(std::env::var_os("KUBECOVE_FRAMES_REPORT").unwrap());
            let path = report.parent().unwrap().join(format!("{}.png", case.id));
            let image = driver
                .cx
                .update_window(driver.handle, |_, window, _| window.render_to_image())
                .unwrap()
                .unwrap();
            timings.gpu_size = Some((image.width(), image.height()));
            image.save(path).unwrap();
        }
    }
    timings
}

pub(super) fn gpu_floor(warmup: usize, frames: usize) -> Vec<u64> {
    let mut cx = FrameContext::new(true);
    let Ok((handle, _)) = cx.update(|cx| {
        gpui_kit::open_window(window_options(), cx, |_, cx| cx.new(|_| gpui_kit::Empty))
    }) else {
        return vec![];
    };
    let mut samples = Vec::with_capacity(frames);
    for index in 0..warmup + frames {
        cx.update_window(handle, |_, window, _| window.refresh())
            .unwrap();
        cx.settle();
        let start = Instant::now();
        if let Err(error) = cx
            .update_window(handle, |_, window, _| window.render_to_image())
            .unwrap()
        {
            eprintln!("empty scene offscreen rendering failed: {error:#}");
            return vec![];
        }
        if index >= warmup {
            samples.push(nanos(start.elapsed()));
        }
    }
    samples
}

#[cfg(feature = "frame-profile")]
fn scroll_profile(driver: &mut Driver, phase: &str) {
    let inspector = driver.inspector.clone();
    let scroll = driver
        .cx
        .update(|cx| inspector.read(cx).frame_scroll_profile());
    let geometry = driver.read(|window| {
        ["detail-body", "events-list", "event:0", "timeline"]
            .into_iter()
            .filter_map(|id| {
                window
                    .try_find(id)
                    .map(|element| (id, format!("{:?}", element.bounds())))
            })
            .collect::<Vec<_>>()
    });
    println!("scroll premise {phase}: {scroll}, {geometry:?}");
}
