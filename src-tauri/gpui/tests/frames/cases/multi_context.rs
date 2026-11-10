//! Multi-context Resources: the 10,000-row watch-burst workload in each of N
//! workspace contexts (`fixture::contexts`). Each context count gets the same
//! cases, so a count is added by listing it in `context_cases!` below.

use super::*;
use crate::fixture::context_name;

/// Scrolls `area` until `target` is in view, then clicks it.
fn click_in(window: &mut Window, cx: &mut App, area: &'static str, target: ElementId) {
    let element = window.find(target.clone());
    if !element.visible() {
        let bounds = window.find(id(area)).bounds();
        let delta = bounds.center().y - element.bounds().center().y;
        wheel(
            window,
            cx,
            bounds.origin + point(px(4.), px(4.)),
            f32::from(delta),
        );
        window.render_frame(cx);
    }
    window.click(target, cx);
}

/// Clicks the one element labeled `label` inside `area`.
fn click_label(window: &mut Window, cx: &mut App, area: &'static str, label: &str) {
    let target = window
        .within(area)
        .find_by_label(label)
        .path()
        .last()
        .expect("labeled element has an ID")
        .clone();
    click_in(window, cx, area, target);
}

/// Adds contexts 1 to `N - 1` to the saved workspace, reopens it and opens
/// Resources once every context's rows are listed.
fn open<const N: usize>(driver: &mut Driver) {
    if N > 1 {
        driver.act(|window, cx| window.click("header-workspaces", cx));
        driver.act(|window, cx| {
            click_label(window, cx, "launcher-body", "Edit Parity mock workspace")
        });
        for rank in 1..N {
            driver.act(|window, cx| {
                let target = id(&format!("workspace-group:{}", context_name(rank)));
                click_in(window, cx, "workspace-groups", target);
            });
        }
        driver.act(|window, cx| window.click("workspace-create", cx));
        driver.act(|window, cx| {
            click_label(window, cx, "launcher-body", "Open Parity mock workspace")
        });
    }
    open_resources(driver);
    driver.cx.settle();
    driver.draw();
    let summary = driver.read(|window| {
        window
            .find(id("resource-table-summary"))
            .label()
            .unwrap_or_default()
            .to_owned()
    });
    assert!(
        summary.starts_with(&format!("{} resources", N * 10_000)),
        "{N} contexts must list {} rows: {summary}",
        N * 10_000
    );
}

fn search<const N: usize>(driver: &mut Driver) {
    open::<N>(driver);
    driver.act(|window, cx| window.click("search", cx));
}

/// Opens the Live menu, unmeasured, so the step can click its refresh item.
fn open_live_menu(driver: &mut Driver, _: usize) {
    if driver.read(|window| window.try_find(id("refresh")).is_none()) {
        driver.act(|window, cx| window.click("live-status", cx));
    }
}

macro_rules! context_cases {
    ($($count:literal),*) => {
        /// The context counts measured, each with every case below.
        pub const COUNTS: &[usize] = &[$($count),*];

        pub static CASES: &[Case] = &[$(
            Case {
                id: concat!("multi-context-", $count),
                description: concat!("Resources page redraw with the 10,000-row watch-burst rows in each of ", $count, " contexts"),
                setup: open::<$count>,
                prepare: None,
                aim: nowhere,
                step: None,
                fixture: Some(concat!("fixture-contexts-", $count, ".json")),
                replay: None,
                probe: None,
            },
            Case {
                id: concat!("multi-context-", $count, "-search"),
                description: concat!("Type `checkout-6` into the resource search, then delete it, with 10,000 rows in each of ", $count, " contexts"),
                setup: search::<$count>,
                prepare: None,
                aim: nowhere,
                step: Some(|window, cx, index, _| typing(window, cx, index, "checkout-6")),
                fixture: Some(concat!("fixture-contexts-", $count, ".json")),
                replay: None,
                probe: Some(|window| value(window, "search")),
            },
            Case {
                id: concat!("multi-context-", $count, "-sort"),
                description: concat!("Click the Name header, flipping its direction, with 10,000 rows in each of ", $count, " contexts"),
                setup: open::<$count>,
                prepare: None,
                aim: |window, _| center(window, "sort:Name"),
                step: Some(click),
                fixture: Some(concat!("fixture-contexts-", $count, ".json")),
                replay: None,
                probe: None,
            },
            Case {
                id: concat!("multi-context-", $count, "-refresh"),
                description: concat!("Click Refresh (clear cache) in the Live menu and settle the reload of every context, with 10,000 rows in each of ", $count, " contexts; the menu opens unmeasured"),
                setup: open::<$count>,
                prepare: Some(open_live_menu),
                aim: |window, _| center(window, "refresh"),
                step: Some(click),
                fixture: Some(concat!("fixture-contexts-", $count, ".json")),
                replay: None,
                probe: None,
            },
        )*];
    };
}

context_cases!(1, 2, 4);
