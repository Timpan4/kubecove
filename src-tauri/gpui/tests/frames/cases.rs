//! The measured views and interactions. Views use the navigation of
//! `tests/visual.rs`. Interactions dispatch raw platform events, because GPUI
//! Kit's click and key helpers draw extra frames around each event, and they
//! locate their target before the timer starts, because `Window::find` walks
//! the whole rendered tree.

use super::Driver;
#[path = "cases/controls.rs"]
mod controls;
#[path = "cases/extra.rs"]
mod extra;
#[path = "cases/interactions.rs"]
mod interactions;
#[path = "cases/navigation.rs"]
mod navigation;
#[path = "cases/views.rs"]
mod views;

pub fn all_cases() -> impl Iterator<Item = &'static Case> {
    CASES
        .iter()
        .chain(extra::CASES)
        .chain(VIEW_CASES)
        .chain(interactions::CASES)
        .chain(navigation::CASES)
        .chain(controls::CASES)
}
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    App, ElementId, InputEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, ScrollDelta, ScrollEvent, Window, point, px,
};

type Step = fn(&mut Window, &mut App, usize, Point<Pixels>);

pub struct Case {
    pub id: &'static str,
    pub description: &'static str,
    pub setup: fn(&mut Driver),
    /// Unmeasured work before a step, such as closing the detail it reopens.
    pub prepare: Option<fn(&mut Driver, usize)>,
    /// Unmeasured: where the input of frame `index` lands, read from the last frame.
    pub aim: fn(&Window, usize) -> Point<Pixels>,
    /// The measured input of frame `index`; views have none and only redraw.
    pub step: Option<Step>,
    /// Alternate recorded workload, relative to the report directory.
    pub fixture: Option<&'static str>,
    /// Sends fixture messages inside the measured state update.
    pub replay: Option<fn(&super::Fixture, &str, usize)>,
    /// State the step must change, beyond the painted quads.
    pub probe: Option<fn(&Window) -> String>,
}

const DETAIL_ROW: &str = "resource:Deployment:payments:payments-api";

fn id(name: &str) -> ElementId {
    ElementId::Name(name.to_owned().into())
}

fn center(window: &Window, target: &str) -> Point<Pixels> {
    window.find(id(target)).bounds().center()
}

fn move_to(
    window: &mut Window,
    cx: &mut App,
    position: Point<Pixels>,
    pressed: Option<MouseButton>,
) {
    window.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: pressed,
            modifiers: Modifiers::default(),
        }
        .to_platform_input(),
        cx,
    );
}

fn mouse_down(window: &mut Window, cx: &mut App, position: Point<Pixels>) {
    window.dispatch_event(
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
}

/// A left click at `position`.
fn click(window: &mut Window, cx: &mut App, _: usize, position: Point<Pixels>) {
    move_to(window, cx, position, None);
    mouse_down(window, cx, position);
    window.dispatch_event(
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
}

fn wheel(window: &mut Window, cx: &mut App, position: Point<Pixels>, delta: f32) {
    move_to(window, cx, position, None);
    window.dispatch_event(
        ScrollEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
}

/// One typed character, or `backspace`, as the platform input handler gets it.
fn key(window: &mut Window, cx: &mut App, key: &str) {
    let mut keystroke =
        Keystroke::parse_with_os(key, cx.platform_info().os).expect("valid keystroke");
    if key.chars().count() == 1 {
        keystroke.key_char = Some(key.to_owned());
    }
    window.dispatch_keystroke(keystroke, cx);
}

/// Types `text`, then deletes it, one keystroke per frame.
fn typing(window: &mut Window, cx: &mut App, index: usize, text: &str) {
    let length = text.chars().count();
    match text.chars().nth(index % (2 * length)) {
        Some(character) => key(window, cx, &character.to_string()),
        None => key(window, cx, "backspace"),
    }
}

/// Wheel steps go one way for `run` frames, then back, so every step scrolls.
fn back_and_forth(index: usize, run: usize, step: f32) -> f32 {
    if (index / run).is_multiple_of(2) {
        -step
    } else {
        step
    }
}

/// A point inside the resource table body, just above its pager.
fn table_point(window: &Window, _: usize) -> Point<Pixels> {
    window.find(id("previous-page")).bounds().origin + point(px(160.), px(-48.))
}

/// A point inside the log list, below the toolbar and filters.
fn log_point(window: &Window, _: usize) -> Point<Pixels> {
    let pause = window.find(id("log-pause")).bounds();
    point(pause.origin.x - px(240.), pause.origin.y + px(220.))
}

/// A point in the ownership map, below its zoom controls.
fn map_point(window: &Window, _: usize) -> Point<Pixels> {
    window.find(id("map-zoom-in")).bounds().origin + point(px(120.), px(160.))
}

fn nowhere(_: &Window, _: usize) -> Point<Pixels> {
    Point::default()
}

fn value(window: &Window, target: &str) -> String {
    window
        .find(id(target))
        .value()
        .unwrap_or_default()
        .to_owned()
}

fn open_resources(driver: &mut Driver) {
    driver.act(|window, cx| window.click("open-resources", cx));
}

/// Pages through the full table to the Deployment's row and scrolls it into view.
fn reveal_detail_row(driver: &mut Driver) {
    while driver.table_page < driver.detail_page {
        driver.act(|window, cx| window.click("next-page", cx));
        driver.table_page += 1;
    }
    // Each step advances half a viewport. Scanning at most the page's row
    // count reaches every row without depending on offscreen elements existing.
    for _ in 0..kubecove_gpui::model::PAGE_SIZE {
        if driver.read(|window| window.try_find(id(DETAIL_ROW)).is_some()) {
            break;
        }
        driver.act(|window, cx| {
            let at = table_point(window, 0);
            let step = window.find(id("resource-table")).bounds().size.height / 2.;
            wheel(window, cx, at, -f32::from(step));
        });
    }
    driver.act(|window, cx| {
        let target = window.find(id(DETAIL_ROW));
        if !target.visible() {
            let anchor = table_point(window, 0);
            let delta = window.viewport_size().height / 2. - target.bounds().center().y;
            wheel(window, cx, anchor, f32::from(delta));
        }
    });
    assert!(
        driver.read(|window| window.find(id(DETAIL_ROW)).visible()),
        "{DETAIL_ROW} must be visible to click"
    );
}

fn open_detail(driver: &mut Driver) {
    open_resources(driver);
    reveal_detail_row(driver);
    driver.act(|window, cx| window.click(DETAIL_ROW, cx));
    assert!(driver.read(|window| window.try_find(id("detail-identity")).is_some()));
}

fn open_tab(driver: &mut Driver, tab: &'static str) {
    open_detail(driver);
    driver.act(|window, cx| window.click(tab, cx));
}

const fn view(id: &'static str, description: &'static str, setup: fn(&mut Driver)) -> Case {
    Case {
        id,
        description,
        setup,
        prepare: None,
        aim: nowhere,
        step: None,
        fixture: None,
        replay: None,
        probe: None,
    }
}

const TABS: &[&str] = &[
    "show-details",
    "show-events",
    "show-logs",
    "show-revisions",
    "show-actions",
    "show-yaml",
];
const EVENT_FILTERS: &[&str] = &[
    "events-filter-warning",
    "events-filter-normal",
    "events-filter-all",
];
const INCIDENT_FILTERS: &[&str] = &[
    "incident-filter:Degraded",
    "incident-filter:Needs attention",
    "incident-filter:Restart evidence",
    "incident-filter:Warnings",
    "incident-filter:All",
];

macro_rules! named_view {
    ($name:literal) => {
        view($name, concat!($name, " view"), |driver| {
            views::setup(driver, $name)
        })
    };
}
static VIEW_CASES: &[Case] = &[
    named_view!("forward-start"),
    named_view!("forward-active"),
    named_view!("exec-session"),
    named_view!("live-limited"),
    named_view!("live-failed"),
    named_view!("live-reconnecting"),
    named_view!("custom-resources"),
    named_view!("network-flow"),
    named_view!("metrics"),
    named_view!("usage-footer"),
    named_view!("actions-image"),
    named_view!("actions-environment"),
    named_view!("actions-resources"),
    named_view!("actions-labels"),
    named_view!("actions-annotations"),
    named_view!("actions-rollback"),
    named_view!("yaml-edit"),
    named_view!("yaml-kyaml"),
    named_view!("yaml-edit-kyaml"),
    named_view!("rbac"),
    named_view!("workspaces"),
    named_view!("settings"),
    named_view!("palette"),
    named_view!("gitops"),
    named_view!("gitops-list"),
    named_view!("flux"),
    named_view!("argo-app"),
    named_view!("argo-workspace"),
    named_view!("helm"),
    named_view!("helm-list"),
    named_view!("helm-release"),
    named_view!("settings-agents"),
    named_view!("settings-appearance"),
    named_view!("settings-sessions"),
    named_view!("settings-yaml"),
    named_view!("settings-diagnostics"),
    named_view!("settings-advanced"),
    named_view!("settings-remote"),
    named_view!("settings-updates"),
    named_view!("settings-kubeconfig"),
    named_view!("argo-connected"),
    named_view!("argo-review"),
    named_view!("argo-desired"),
    named_view!("workspace-editor"),
    named_view!("custom-search"),
    named_view!("kind-picker"),
];

pub static CASES: &[Case] = &[
    view("overview", "Overview page", |_| {}),
    view(
        "resources",
        "Resources page: map and full table",
        open_resources,
    ),
    view("detail", "Deployment detail, Details tab", open_detail),
    view("yaml", "Deployment detail, YAML tab", |d| {
        open_tab(d, "show-yaml")
    }),
    view("events", "Deployment detail, Events tab", |d| {
        open_tab(d, "show-events")
    }),
    view("logs", "Deployment detail, Logs tab", |d| {
        open_tab(d, "show-logs")
    }),
    view("revisions", "Deployment detail, Revisions tab", |d| {
        open_tab(d, "show-revisions")
    }),
    view("actions", "Deployment detail, Actions tab", |d| {
        open_tab(d, "show-actions")
    }),
    view("cluster-overview", "Cluster Overview page", |d| {
        d.act(|w, cx| w.click("nav:Cluster Overview", cx))
    }),
    view("incidents", "Incidents page", |d| {
        d.act(|w, cx| w.click("nav:Incidents", cx))
    }),
    view("port-forwards", "Port Forwards page", |d| {
        d.act(|w, cx| w.click("nav:Port Forwards", cx))
    }),
    Case {
        id: "table-scroll",
        description: "Wheel-scroll the resource table 40px per frame, 30 frames each way",
        setup: open_resources,
        prepare: None,
        aim: table_point,
        step: Some(|w, cx, i, at| wheel(w, cx, at, back_and_forth(i, 30, 40.))),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "log-scroll",
        description: "Wheel-scroll the log list 40px per frame, 30 frames each way",
        setup: |d| open_tab(d, "show-logs"),
        prepare: None,
        aim: log_point,
        step: Some(|w, cx, i, at| wheel(w, cx, at, back_and_forth(i, 30, -40.))),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "detail-open",
        description: "Click a table row to open its detail; it closes, unmeasured, before each click",
        setup: |d| {
            open_resources(d);
            reveal_detail_row(d);
        },
        prepare: Some(|d, _| {
            if d.read(|w| w.try_find(id("close-detail")).is_some()) {
                d.act(|w, cx| w.click("close-detail", cx));
                reveal_detail_row(d);
            }
        }),
        aim: |w, _| center(w, DETAIL_ROW),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "detail-tabs",
        description: "Switch detail tabs: Details, Events, Logs, Revisions, Actions, YAML",
        setup: open_detail,
        prepare: None,
        aim: |w, i| center(w, TABS[i % TABS.len()]),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "search-typing",
        description: "Type `payments-api` into the resource search, then delete it",
        setup: |d| {
            open_resources(d);
            d.act(|w, cx| w.click("search", cx));
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "payments-api")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "search")),
    },
    Case {
        id: "log-filter-typing",
        description: "Type `healthz` into the log filter, then delete it",
        setup: |d| {
            open_tab(d, "show-logs");
            d.act(|w, cx| w.click("log-filter", cx));
        },
        prepare: None,
        aim: nowhere,
        step: Some(|w, cx, i, _| typing(w, cx, i, "healthz")),
        fixture: None,
        replay: None,
        probe: Some(|w| value(w, "log-filter")),
    },
    Case {
        id: "event-filters",
        description: "Toggle event filters: Warnings, Normal, All",
        setup: |d| open_tab(d, "show-events"),
        prepare: None,
        aim: |w, i| center(w, EVENT_FILTERS[i % EVENT_FILTERS.len()]),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "incident-filters",
        description: "Toggle incident filters: Degraded, Needs attention, Restart evidence, Warnings, All",
        setup: |d| d.act(|w, cx| w.click("nav:Incidents", cx)),
        prepare: None,
        aim: |w, i| center(w, INCIDENT_FILTERS[i % INCIDENT_FILTERS.len()]),
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "map-pan",
        description: "Drag the ownership map 8px per frame, 10 frames each way",
        setup: |d| {
            open_resources(d);
            d.act(|w, cx| {
                let at = map_point(w, 0);
                move_to(w, cx, at, None);
                mouse_down(w, cx, at);
            });
        },
        prepare: None,
        aim: map_point,
        step: Some(|w, cx, i, at| {
            // The offset from the drag's start sweeps 0..80px and back.
            let phase = i % 20;
            let offset = if phase < 10 { phase } else { 20 - phase } as f32 * 8.;
            move_to(
                w,
                cx,
                at + point(px(offset), px(offset / 2.)),
                Some(MouseButton::Left),
            );
        }),
        fixture: None,
        replay: None,
        probe: None,
    },
    Case {
        id: "map-zoom",
        description: "Click the map's zoom in three times, then zoom out three times",
        setup: open_resources,
        prepare: None,
        aim: |w, i| {
            center(
                w,
                if i % 6 < 3 {
                    "map-zoom-in"
                } else {
                    "map-zoom-out"
                },
            )
        },
        step: Some(click),
        fixture: None,
        replay: None,
        probe: None,
    },
];
