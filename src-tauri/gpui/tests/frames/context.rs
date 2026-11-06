//! Production App scheduling on headless SDK benchmark windows.

use super::platform::FramePlatform;
use gpui_kit::{
    AnyView, AnyWindowHandle, App, AppContext, Application, ApplicationHandle, Result, Window,
    profiler::journal::{
        ForegroundEvent, ForegroundJournalCollector, ForegroundJournalEntry, FrameSkipReason,
        FrameStateChange, IntervalBoundary,
    },
};
use kubecove_gpui::assets::AppAssets;
use serde::Serialize;
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuFrame {
    pub cpu: u64,
    pub draw: u64,
    pub request_layout: u64,
    pub prepaint: u64,
    pub paint: u64,
    pub submit: u64,
    pub presentation_only: bool,
}

struct Capture {
    scheduler: Option<[u64; 2]>,
    started: Instant,
    content_settled: Instant,
    work: Duration,
    frames: Vec<CpuFrame>,
}

pub struct FrameContext {
    app: ApplicationHandle,
    platform: Rc<FramePlatform>,
    journal: ForegroundJournalCollector,
    capture: Option<Capture>,
}

impl FrameContext {
    pub fn new(gpu: bool) -> Self {
        let platform = FramePlatform::new(gpu);
        let app = Application::with_platform(platform.clone())
            .with_assets(AppAssets)
            .run_embedded(|cx| {
                gpui_kit::init(cx);
                kubecove_gpui::init(cx).expect("fonts load");
            });
        let journal = app.update(|cx| cx.foreground_journal().collector());
        Self {
            app,
            platform,
            journal,
            capture: None,
        }
    }

    pub fn update<R>(&mut self, update: impl FnOnce(&mut App) -> R) -> R {
        self.app.update(update)
    }

    pub fn update_window<R>(
        &mut self,
        handle: AnyWindowHandle,
        update: impl FnOnce(AnyView, &mut Window, &mut App) -> R,
    ) -> Result<R> {
        self.operation(false, |app| {
            app.update(|cx| cx.update_window(handle, update))
        })
        .0
    }

    pub fn begin_sample(&mut self) {
        assert!(self.capture.is_none());
        self.journal.collect_unseen();
        let scheduler = scheduler();
        let started = Instant::now();
        self.capture = Some(Capture {
            scheduler,
            started,
            content_settled: started,
            work: Duration::ZERO,
            frames: Vec::new(),
        });
    }

    pub fn finish_sample(&mut self) -> (u64, Vec<CpuFrame>, Option<[u64; 2]>) {
        let mut capture = self.capture.take().expect("active measurement");
        let scheduler = capture
            .scheduler
            .zip(scheduler())
            .and_then(|(before, after)| {
                Some([
                    after[0].checked_sub(before[0])?,
                    after[1].checked_sub(before[1])?,
                ])
            });
        if let Some(last) = capture.frames.last_mut() {
            last.cpu += super::nanos(capture.work);
        }
        (
            super::nanos(capture.content_settled.duration_since(capture.started)),
            capture.frames,
            scheduler,
        )
    }

    // Native key dispatch can draw before processing the key. Split CPU work at
    // each SDK draw completion, carrying later state work into the next frame.
    fn operation<R>(
        &mut self,
        callback: bool,
        operation: impl FnOnce(&ApplicationHandle) -> R,
    ) -> (R, bool, bool) {
        let start = Instant::now();
        let result = operation(&self.app);
        let end = Instant::now();
        let entries = self.journal.collect_unseen();
        assert_eq!(entries.lost, 0, "frame journal lost measurement events");
        let mut draws = Vec::new();
        let mut presented = false;
        let mut newly_presented = false;
        let mut submit = Duration::ZERO;
        let mut pending = false;
        for entry in entries.entries {
            match entry {
                ForegroundJournalEntry::Event(ForegroundEvent::Draw(draw)) => draws.push(draw),
                ForegroundJournalEntry::Event(ForegroundEvent::Present(timing)) => {
                    presented = true;
                    submit += timing.present_end.duration_since(timing.present_start);
                }
                ForegroundJournalEntry::Boundary(IntervalBoundary::Presented(frame)) => {
                    newly_presented = true;
                    submit += frame
                        .presentation
                        .present_end
                        .duration_since(frame.presentation.present_start);
                }
                ForegroundJournalEntry::FrameState(FrameStateChange::Pending { .. }) => {
                    pending = true
                }
                ForegroundJournalEntry::Boundary(IntervalBoundary::FrameSkipped(skipped)) => {
                    assert_eq!(
                        skipped.reason,
                        FrameSkipReason::NoRenderNeeded,
                        "measurement window was throttled"
                    );
                }
                _ => {}
            }
        }
        draws.sort_unstable_by_key(|draw| draw.draw_end);
        let drew = !draws.is_empty();
        if let Some(capture) = &mut self.capture {
            let mut cursor = start;
            for draw in draws {
                capture.work += draw.draw_end.duration_since(cursor);
                cursor = draw.draw_end;
                capture.frames.push(CpuFrame {
                    cpu: super::nanos(capture.work),
                    draw: super::nanos(draw.draw_end.duration_since(draw.draw_start)),
                    request_layout: super::nanos(draw.phases.request_layout),
                    prepaint: super::nanos(draw.phases.prepaint),
                    paint: super::nanos(draw.phases.paint),
                    presentation_only: false,
                    submit: 0,
                });
                capture.work = Duration::ZERO;
                capture.content_settled = cursor;
            }
            capture.work += end.duration_since(cursor);
            if callback && drew {
                let last = capture.frames.last_mut().unwrap();
                last.submit += super::nanos(submit);
                last.cpu += super::nanos(capture.work);
                capture.work = Duration::ZERO;
                capture.content_settled = end;
            } else if callback && newly_presented {
                // Native key dispatch may draw synchronously. Its later submit
                // completes that content frame rather than a VRR keepalive.
                let last = capture
                    .frames
                    .iter_mut()
                    .rev()
                    .find(|frame| !frame.presentation_only)
                    .expect("a newly submitted scene has a measured draw");
                last.cpu += super::nanos(capture.work);
                last.submit += super::nanos(submit);
                capture.work = Duration::ZERO;
                capture.content_settled = end;
            } else if callback && presented {
                capture.frames.push(CpuFrame {
                    cpu: super::nanos(capture.work),
                    draw: 0,
                    request_layout: 0,
                    prepaint: 0,
                    paint: 0,
                    presentation_only: true,
                    submit: super::nanos(submit),
                });
                capture.work = Duration::ZERO;
            }
        }
        (result, drew || newly_presented, pending)
    }

    /// One frame turn, for steady views and unmeasured setup. Ongoing frame
    /// demand carries into the next turn rather than blocking a frame sample.
    pub fn tick(&mut self) -> (bool, bool) {
        let executor = self.app.update(|cx| cx.background_executor().clone());
        self.operation(false, |_| {
            let _span = kubecove_gpui::frame_profile::scope("dispatcher", "drain");
            executor
                .dispatcher()
                .as_threaded()
                .expect("benchmark dispatcher")
                .run_until_idle();
        });
        let open = self.app.update(|cx| cx.windows());
        let mut drew = false;
        let mut dispatched = false;
        for callback in self.platform.callbacks(&open) {
            let (ran, rendered, _) = self.operation(true, |_| callback());
            dispatched |= ran;
            drew |= rendered;
        }
        (dispatched, drew)
    }

    /// Settle content, including follow-up draws. A renderless completion
    /// callback establishes content quiescence. VRR keepalive presentation
    /// is sampled too, but does not extend content-settled latency.
    pub fn settle(&mut self) {
        let executor = self.app.update(|cx| cx.background_executor().clone());
        loop {
            let (dispatched, drew) = self.tick();
            if !dispatched {
                break;
            }
            if !drew {
                let (_, rendered, pending) = self.operation(false, |_| {
                    let _span = kubecove_gpui::frame_profile::scope("dispatcher", "drain");
                    executor
                        .dispatcher()
                        .as_threaded()
                        .expect("benchmark dispatcher")
                        .run_until_idle();
                });
                if !rendered && !pending {
                    break;
                }
            }
        }
    }
}

impl Drop for FrameContext {
    fn drop(&mut self) {
        self.app.update(|cx| cx.shutdown());
    }
}

/// Linux scheduler counters for this UI thread, sampled outside CPU work timers.
/// The pair is running CPU nanoseconds and time waiting on a runqueue.
fn scheduler() -> Option<[u64; 2]> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/thread-self/schedstat").ok()?;
        let mut fields = text.split_ascii_whitespace();
        Some([fields.next()?.parse().ok()?, fields.next()?.parse().ok()?])
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
