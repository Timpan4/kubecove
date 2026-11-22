//! The developer topology harness at 4,000 synthetic nodes. Release builds
//! have no Diagnostics entry for it, so setup opens it on the Inspector.

use super::*;
use kubecove_gpui::Inspector;

/// Diagnostics' "Open 4k LOD" query.
const LOD_4K: &str =
    "nodes=4000&viewport=focused&nodeDetail=compact&edgeMode=selected&edgeType=straight";
/// The harness's heaviest options at the same size: full nodes and every edge, fitted.
const FULL_4K: &str = "nodes=4000&viewport=fit&nodeDetail=full&edgeMode=all&edgeType=smoothstep";

fn open(driver: &mut Driver, query: &'static str) {
    driver
        .cx
        .update_window(driver.handle, |_, window, cx| {
            let inspector = gpui_kit::base::Root::read(window, cx)
                .view()
                .clone()
                .downcast::<Inspector>()
                .expect("the Inspector is the window's content");
            inspector.update(cx, |inspector, cx| {
                inspector.open_topology_harness(query, window, cx)
            });
        })
        .unwrap();
    driver.cx.tick();
    // The fitted viewport needs the canvas size from the first frame.
    driver.draw();
    driver.draw();
}

/// A point on the harness canvas, away from its controls in the lower left.
fn canvas_point(window: &Window, _: usize) -> Point<Pixels> {
    center(window, "topology-spike-canvas")
}

fn start_drag(driver: &mut Driver) {
    driver.act(|window, cx| {
        let at = canvas_point(window, 0);
        move_to(window, cx, at, None);
        mouse_down(window, cx, at);
    });
}

/// The map-pan sweep: 8px per frame, 10 frames each way.
fn pan(window: &mut Window, cx: &mut App, index: usize, at: Point<Pixels>) {
    let phase = index % 20;
    let offset = if phase < 10 { phase } else { 20 - phase } as f32 * 8.;
    move_to(
        window,
        cx,
        at + point(px(offset), px(offset / 2.)),
        Some(MouseButton::Left),
    );
}

pub static CASES: &[Case] = &[
    Case {
        id: "topology-spike-pan",
        description: "Open 4k LOD in the topology harness and drag its focused viewport 8px per frame",
        setup: |driver| {
            open(driver, LOD_4K);
            start_drag(driver);
        },
        prepare: None,
        aim: canvas_point,
        step: Some(pan),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "topology-spike-fit-pan",
        description: "Open 4k LOD, press Fit View, and drag the whole graph at minimum zoom 8px per frame",
        setup: |driver| {
            open(driver, LOD_4K);
            driver.act(|window, cx| window.click("topology-spike-fit", cx));
            start_drag(driver);
        },
        prepare: None,
        aim: canvas_point,
        step: Some(pan),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "topology-spike-full-pan",
        description: "Open the harness with 4,000 full nodes and every smooth-step edge, fitted, and drag it 8px per frame",
        setup: |driver| {
            open(driver, FULL_4K);
            start_drag(driver);
        },
        prepare: None,
        aim: canvas_point,
        step: Some(pan),
        fixture: None,
        replay: None,
        probe: None,
    },
];
