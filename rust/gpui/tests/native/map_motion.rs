//! Failure modes: the dashes of a selected path are redrawn at the same offset;
//! the step timer fires once and the next step is never arranged; a pulse stays
//! at one point of its edge. No step here forces a render: each frame has to
//! come from the motion's own wake, on GPUI's test clock. The shared scenarios
//! in `e2e/parity/scenarios/map-motion.json` cover when edges move at all.
use super::map_live::Snapshot;
use super::*;
use std::time::{Duration, Instant};

/// One frame of a 60 Hz display.
const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

fn open_map(cx: &mut TestAppContext, snapshot: Snapshot) -> gpui_kit::AnyWindowHandle {
    let backend = Arc::new(Controlled::default());
    backend.map_live.show(snapshot);
    let handle = open(cx, backend);
    cx.update_window(handle, |_, window, cx| {
        // Edges move only in the active window, and the test window starts inactive.
        window.activate_window();
        // The first frame measures the graph viewport; the second fits it.
        window.render_frame(cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    handle
}

/// The last frame's dash phase, pulse phase and next dash wake.
fn clock(
    cx: &mut TestAppContext,
    handle: gpui_kit::AnyWindowHandle,
) -> (f32, Option<f32>, Option<Instant>) {
    cx.update_window(handle, |_, window, cx| {
        gpui_kit::base::Root::read(window, cx)
            .view()
            .clone()
            .downcast::<Inspector>()
            .expect("the Inspector is the window's content")
            .read(cx)
            .map_edge_clock()
    })
    .unwrap()
    .expect("an edge moves")
}

#[gpui_kit::test]
fn a_selected_paths_dashes_advance_one_timer_step_after_another(cx: &mut TestAppContext) {
    let handle = open_map(cx, Snapshot::ReplicaSet);
    cx.update_window(handle, |_, window, cx| {
        window.click(format!("map-node:{MAP_NODE}"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    let mut last = clock(cx, handle);
    // The second step proves that a frame drawn by the timer arranges the next one.
    for _ in 0..2 {
        let wake = last.2.expect("dashes alone wake from a timer");
        let now = cx.background_executor.now();
        cx.background_executor.advance_clock(wake - now);
        cx.run_until_parked();
        let next = clock(cx, handle);
        assert_ne!(next.0, last.0, "the timer's frame draws the dashes further");
        last = next;
    }
}

#[gpui_kit::test]
fn a_pulse_moves_along_its_edge_from_frame_to_frame(cx: &mut TestAppContext) {
    let handle = open_map(cx, Snapshot::RollingOut);
    let mut last = clock(cx, handle).1.expect("live updates are current");
    for _ in 0..2 {
        cx.background_executor.advance_clock(FRAME);
        cx.update_window(handle, |_, window, cx| window.simulate_next_frame(cx))
            .unwrap();
        cx.run_until_parked();
        let next = clock(cx, handle).1.expect("live updates are current");
        assert_ne!(next, last, "the frame the pulse asked for draws it further");
        last = next;
    }
}
