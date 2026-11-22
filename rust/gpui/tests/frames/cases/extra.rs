//! Additional controls on the virtualized detail surfaces.

use super::*;

fn open_timeline(driver: &mut Driver) {
    open_detail(driver);
    driver.act(|window, cx| {
        let target = window.find(id("timeline-expand-all"));
        if !target.visible() {
            let delta = window.viewport_size().height / 2. - target.bounds().center().y;
            let at = center(window, "show-details") + point(px(0.), px(120.));
            wheel(window, cx, at, f32::from(delta));
        }
    });
    assert!(driver.read(|window| window.find(id("timeline-expand-all")).visible()));
}

fn page_point(window: &Window, _: usize) -> gpui_kit::Point<gpui_kit::Pixels> {
    let size = window.viewport_size();
    point(size.width / 2., size.height / 2.)
}

pub static CASES: &[Case] = &[
    Case {
        id: "exec-session-scroll",
        description: "Wheel-scroll the exec sessions 40px per sample, starting at the page bottom",
        setup: |driver| {
            driver.act(|window, cx| window.click("nav:Port Forwards", cx));
            driver.act(|window, cx| {
                let rows: usize = super::super::fixture::SIZES
                    .iter()
                    .filter(|(name, _, _)| {
                        *name == "port-forward sessions" || *name == "exec sessions"
                    })
                    .map(|(_, count, _)| count)
                    .sum();
                wheel(
                    window,
                    cx,
                    page_point(window, 0),
                    -f32::from(window.viewport_size().height) * rows as f32,
                );
            });
        },
        prepare: None,
        aim: page_point,
        step: Some(|window, cx, index, at| wheel(window, cx, at, back_and_forth(index, 30, -40.))),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "detail-close",
        description: "Close the detail; reopen it unmeasured before each click",
        setup: open_detail,
        prepare: Some(|driver, _| {
            if driver.read(|window| window.try_find(id("close-detail")).is_none()) {
                reveal_detail_row(driver);
                driver.act(|window, cx| window.click(DETAIL_ROW, cx));
            }
        }),
        aim: |window, _| center(window, "close-detail"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "timeline-toggle-all",
        description: "Expand and collapse all timeline entries, with the timeline in view",
        setup: open_timeline,
        prepare: None,
        aim: |window, index| {
            center(
                window,
                if index.is_multiple_of(2) {
                    "timeline-expand-all"
                } else {
                    "timeline-collapse-all"
                },
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "events-search-typing",
        description: "Type `replica` into the events search, then delete it",
        setup: |driver| {
            open_tab(driver, "show-events");
            driver.act(|window, cx| window.click("events-search", cx));
        },
        prepare: None,
        aim: nowhere,
        step: Some(|window, cx, index, _| typing(window, cx, index, "replica")),
        fixture: None,
        replay: None,
        probe: Some(|window| value(window, "events-search")),
    },
    Case {
        id: "log-wrap",
        description: "Toggle wrapping of the 1,000 retained log lines",
        setup: |driver| open_tab(driver, "show-logs"),
        prepare: None,
        aim: |window, _| center(window, "log-wrap"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "log-order",
        description: "Toggle newest-first and oldest-first log order",
        setup: |driver| open_tab(driver, "show-logs"),
        prepare: None,
        aim: |window, _| center(window, "log-order"),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
];
