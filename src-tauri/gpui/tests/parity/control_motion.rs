//! `motion:<target>:<phase>` checks: a control moves to its new state over
//! `control_motion::DURATION` instead of jumping. `start` changes the control's state and
//! expects the transition at one end; `midway` expects it between the ends after
//! the scenario advanced the clock by half the duration; `settled` expects it over.
//! After `midway`, `back` chooses the first state again and expects the indicator
//! where it was shown, and `returned` expects it at rest. `instant`, run under
//! reduced motion, changes the state and expects it there in the same frame.
//! Mirrors `e2e/parity/web-control-motion.ts`.
use super::*;
use gpui_kit::ElementId;
use gpui_kit::test::TestEventExt;
use kubecove_gpui::control_motion::Moving;
use std::cell::RefCell;

const SETTING: &str = "Show exact timestamps";
const DEPLOYMENT: &str = "resource:Deployment:payments:payments-api";

/// What the earlier phases of the running check saw.
struct Seen {
    /// The progress at `start`.
    progress: f32,
    /// The switch thumb's left edge at `start`.
    thumb: f32,
    /// The target's paint in each phase so far.
    paint: Vec<Vec<String>>,
}

thread_local! {
    static SEEN: RefCell<Option<Seen>> = const { RefCell::new(None) };
}

pub(super) fn handles(check: &str) -> bool {
    check.starts_with("motion:")
}

pub(super) fn check(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, check: &str) {
    let mut parts = check.split(':').skip(1);
    let (target, phase) = (parts.next().unwrap(), parts.next().unwrap());
    if phase == "instant" {
        let before = paint(window, cx, target);
        let moving = change(window, cx, target);
        window.render_frame(cx);
        assert_eq!(
            Moving::progress(&moving, cx),
            None,
            "{target} moves under reduced motion"
        );
        assert_ne!(
            paint(window, cx, target),
            before,
            "{target} did not change under reduced motion"
        );
        return;
    }
    if phase == "start" {
        let moving = change(window, cx, target);
        window.render_frame(cx);
        let progress = Moving::progress(&moving, cx)
            .unwrap_or_else(|| panic!("{target} changed without a transition"));
        assert!(
            progress == 0. || progress == 1.,
            "{target} jumped to {progress}"
        );
        SEEN.set(Some(Seen {
            progress,
            thumb: thumb(window, target),
            paint: paint(window, cx, target).into_iter().collect(),
        }));
        return;
    }
    if phase == "back" || phase == "returned" {
        if phase == "back" {
            match target {
                "segment" => window.click("gitops-view:Cards", cx),
                "tab" => window.click("show-details", cx),
                other => panic!("{other} has no first state to turn back to"),
            }
            window.render_frame(cx);
        }
        let moving = transition(window, cx, target);
        let progress = Moving::progress(&moving, cx);
        let painted = paint(window, cx, target).expect("the indicator paints quads");
        SEEN.with_borrow(|seen| {
            let seen = seen.as_ref().expect("a start check ran first");
            let [start, midway] = &seen.paint[..] else {
                panic!("{check} needs a start and a midway check first");
            };
            // The quads only one phase painted are the indicator in that phase:
            // on its way at `midway` and at rest at `start`.
            let (shown, other, place) = if phase == "back" {
                assert_eq!(progress, Some(0.), "{target} did not turn back in place");
                (midway, start, "where it was shown")
            } else {
                assert_eq!(progress, None, "{target} still moves after its duration");
                (start, midway, "at rest")
            };
            let only = |of: &[String], not: &[String]| -> Vec<String> {
                of.iter().filter(|quad| !not.contains(quad)).cloned().collect()
            };
            let indicator = only(shown, other);
            assert!(!indicator.is_empty(), "{target} paints no indicator");
            for quad in &indicator {
                assert!(
                    painted.contains(quad),
                    "{target} is not {place} at {phase}: {quad} is missing from {painted:?}"
                );
            }
            // Nor is the indicator also left where the other phase showed it.
            for quad in only(other, shown) {
                assert!(
                    !painted.contains(&quad),
                    "{target} still paints {quad} at {phase}, as it did away from {place}"
                );
            }
        });
        return;
    }
    let moving = transition(window, cx, target);
    let progress = Moving::progress(&moving, cx);
    let painted = paint(window, cx, target);
    let (start, thumb_start) = SEEN.with_borrow_mut(|seen| {
        let seen = seen.as_mut().expect("a start check ran first");
        // A transition that advances without reaching the screen paints one
        // phase as it painted another.
        if let Some(painted) = painted {
            assert!(
                !seen.paint.contains(&painted),
                "{target} paints at {phase} as it did in an earlier phase: {painted:?}"
            );
            seen.paint.push(painted);
        }
        (seen.progress, seen.thumb)
    });
    match phase {
        "midway" => {
            let progress = progress.unwrap_or_else(|| panic!("{target} settled at half time"));
            assert!(
                progress > 0. && progress < 1. && progress != start,
                "{target} is at {progress} at half time"
            );
            if target == "switch" {
                let moved = (thumb(window, target) - thumb_start).abs();
                // `translate-x-4` moves the thumb 16 px.
                assert!(moved > 0. && moved < 16., "the thumb moved {moved} px");
            }
        }
        "settled" => {
            assert_eq!(progress, None, "{target} still moves after its duration");
            if target == "switch" {
                assert_eq!((thumb(window, target) - thumb_start).abs(), 16.);
            }
        }
        other => panic!("unknown motion phase {other}"),
    }
}

/// The quads painted inside the target: their bounds and colors. `None` for the
/// chevron, which is a sprite, and the scene exposes only quads.
fn paint(
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
    target: &str,
) -> Option<Vec<String>> {
    // A pill or a line slides from the option that was active to the chosen one.
    let (id, from): (ElementId, Option<&'static str>) = match target {
        "switch" => (ElementId::Name(format!("setting:{SETTING}").into()), None),
        "segment" => ("gitops-view:List".into(), Some("gitops-view:Cards")),
        "tab" => ("show-events".into(), Some("show-details")),
        "chevron" => return None,
        // A fold paints nothing of its own. The row under the branch shows it.
        "branch" => {
            let below = window.find("nav:Network").bounds().origin.y;
            return Some(vec![format!("{below:?}")]);
        }
        _ => (transition(window, cx, target), None),
    };
    let mut area = window.find(id).bounds();
    if let Some(from) = from {
        area = area.union(&window.find(from).bounds());
    }
    // The tab's line sits 4 px under its trigger.
    let area = area.dilate(gpui_kit::px(4.)).scale(window.scale_factor());
    let edge = area.bottom_right();
    Some(
        window
            .painted_quads()
            .into_iter()
            .filter(|quad| {
                // The line's lower edge lies on the area's, which
                // `is_contained_within` leaves out.
                let (start, end) = (quad.bounds.origin, quad.bounds.bottom_right());
                start.x >= area.origin.x
                    && start.y >= area.origin.y
                    && end.x <= edge.x
                    && end.y <= edge.y
            })
            .map(|quad| format!("{:?} {:?}", quad.bounds, quad.background))
            .collect(),
    )
}

/// The switch thumb's left edge, or 0 for a target without a moving element.
fn thumb(window: &gpui_kit::Window, target: &str) -> f32 {
    if target != "switch" {
        return 0.;
    }
    f32::from(
        window
            .find(format!("setting-thumb:{SETTING}"))
            .bounds()
            .origin
            .x,
    )
}

/// Changes the target's state and returns the id its transition reports under.
fn change(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, target: &str) -> ElementId {
    match target {
        "switch" => locate::click_in(window, "page", format!("setting:{SETTING}").into(), cx),
        "segment" => window.click("gitops-view:List", cx),
        "tab" => window.click("show-events", cx),
        "row" => {
            let row = transition(window, cx, target);
            let position = window.find(row).bounds().center();
            window.pointer_move(position, None, cx);
        }
        "button" => {
            reveal(window, "detail-body", "edit-draft", cx);
            let position = window.find("edit-draft").bounds().center();
            window.pointer_move(position, None, cx);
        }
        "chevron" => {
            reveal(window, "sidebar-tree", "tree-toggle:Workloads", cx);
            sidebar_live::click_sidebar(window, cx, "tree-toggle:Workloads".into());
        }
        "branch" => {
            // The runner opens branches at once in every other scenario.
            Moving::set_instant_layout(false, cx);
            reveal(window, "sidebar-tree", "tree-toggle:Workloads", cx);
            sidebar_live::click_sidebar(window, cx, "tree-toggle:Workloads".into());
        }
        other => panic!("unknown motion target {other}"),
    }
    transition(window, cx, target)
}

/// The id the target's transition reports under: the `eased` id, the sliding
/// indicator's id, the id of the element a `hover_fill` fills, or a fold's path.
fn transition(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, target: &str) -> ElementId {
    let id = match target {
        "switch" => format!("switch-thumb:{SETTING}"),
        "segment" => "segment-pill:gitops-view".into(),
        "tab" => "tab-line".into(),
        "row" => {
            let name = detail::resolve_resource(window, DEPLOYMENT, cx)
                .expect("the resource table shows the Deployment");
            format!("row:{name}")
        }
        "button" => "edit-draft".into(),
        "chevron" => "chevron:Workloads".into(),
        "branch" => "fold:Workloads".into(),
        other => panic!("unknown motion target {other}"),
    };
    ElementId::Name(id.into())
}
