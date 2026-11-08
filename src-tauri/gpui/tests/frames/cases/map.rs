//! Full redraws of a fanned-out ownership map. The scaled 4,000-node fixture
//! keeps each copy's edges local, so culling leaves a few dozen of them on
//! screen at any zoom. The fan-out workload (`fixture::fan_out`) adds the
//! memory profile's 10,000 Pods under one ReplicaSet, whose edges all cross the
//! viewport. This is the redraw that perf-map.md measured at 60 to 120 ms per
//! notify in the real app at 10,000 Pods.

use super::*;

/// The map's zoom, as the viewport element reports it to two decimals.
fn zoom(window: &Window) -> String {
    value(window, "map-viewport")
}

pub static CASES: &[Case] = &[Case {
    id: "map-redraw-large",
    description: "Redraw the ownership map with 10,000 Pods under one ReplicaSet (e2e/harness/memory-fixture.ts `large`), zoomed out to its minimum",
    setup: |driver| {
        open_resources(driver);
        // Each press scales by `ZOOM_STEP` until the zoom stops at `MIN_ZOOM`.
        loop {
            let before = driver.read(|window| zoom(window));
            driver.act(|window, cx| window.click("map-zoom-out", cx));
            if driver.read(|window| zoom(window)) == before {
                break;
            }
        }
    },
    prepare: None,
    aim: nowhere,
    step: None,
    fixture: Some("fixture-fan-out.json"),
    replay: None,
    probe: None,
}];
