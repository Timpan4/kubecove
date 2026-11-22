//! `motion:<target>:<phase>` checks: a control moves to its new state over
//! `control_motion::DURATION` instead of jumping. `start` changes the control's state and
//! expects the transition at one end; `midway` expects it between the ends after
//! the scenario advanced the clock by half the duration; `settled` expects it over.
//! After `midway`, `back` chooses the first state again and expects the indicator
//! where it was shown, and `returned` expects it at rest. `instant`, run under
//! reduced motion, changes the state and expects it there in the same frame.
//! `ended`, after reduced motion turned on mid-run, expects what `settled` does
//! without moving the clock. `collapsed`, after another action closed a fold
//! mid-run, expects the fold over and the branch closed without moving the clock.
//! `inert`, mid-fold, expects Tab and a click to pass over the branch's rows.
//! Mirrors `e2e/parity/web-control-motion.ts`.
use super::*;
use gpui_kit::ElementId;
use gpui_kit::test::TestEventExt;
use kubecove_gpui::control_motion::Moving;
use kubecove_gpui::theme::Tokens;
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
        let moving = change(window, cx, target);
        window.render_frame(cx);
        assert_eq!(
            Moving::progress(&moving, cx),
            None,
            "{target} moves under reduced motion"
        );
        // The indicator's color, and where it rests under the chosen option: the
        // option's box for a pill, a 2 px line 2 px under it for the tab line, as
        // `inspector/detail.rs` and `inspector/controls.rs` build them.
        let (option, color, line) = match target {
            "segment" => ("gitops-view:List", Tokens::card(), false),
            "tab" => ("show-events", Tokens::foreground(), true),
            other => panic!("{other} has no reduced-motion check"),
        };
        let mut rest = window.find(option).bounds();
        if line {
            rest.origin.y += rest.size.height + gpui_kit::px(2.);
            rest.size.height = gpui_kit::px(2.);
        }
        let rest = format!("{:?}", rest.scale(window.scale_factor()));
        let indicators: Vec<_> = quads(window, cx, target)
            .into_iter()
            .filter(|quad| quad.background == color.into())
            .map(|quad| format!("{:?}", quad.bounds))
            .collect();
        assert_eq!(
            indicators,
            [rest],
            "{target} does not show one indicator at rest under its chosen option"
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
    // A fold another action closed, such as Collapse all, shows the branch
    // closed at once, without the clock moving.
    if phase == "collapsed" {
        window.render_frame(cx);
        let moving = transition(window, cx, target);
        assert_eq!(
            Moving::progress(&moving, cx),
            None,
            "{target} still folds after it was collapsed"
        );
        let painted = paint(window, cx, target).expect("a fold paints the row under it");
        SEEN.with_borrow(|seen| {
            let rest = &seen.as_ref().expect("a start check ran first").paint[0];
            assert_eq!(&painted, rest, "{target} is not closed at once");
        });
        return;
    }
    // A folding branch's rows take no focus or clicks, as the web's sliding
    // list is inert. Tab starts from the row whose chevron started the fold.
    if phase == "inert" {
        let first = match target {
            "branch" => "tree:Workloads/Pod",
            other => panic!("{other} has no inert check"),
        };
        window.press("tab", cx);
        window.render_frame(cx);
        assert_ne!(
            window.find(first).focused(),
            Some(true),
            "Tab reached a row of {target} while it folds"
        );
        window.click(first, cx);
        window.render_frame(cx);
        assert_ne!(
            window.find(first).selected(),
            Some(true),
            "a click selected a row of {target} while it folds"
        );
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
                of.iter()
                    .filter(|quad| !not.contains(quad))
                    .cloned()
                    .collect()
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
        // A fold moves the row under its branch one way: it passes no end
        // and then snaps back to it.
        if let ([start, midway, settled], Some(_)) = (&seen.paint[..], row_below(target)) {
            let [start, midway, settled] =
                [start, midway, settled].map(|paint| paint[0].parse::<f32>().unwrap());
            assert!(
                (start - midway) * (midway - settled) > 0.,
                "the row under {target} moved {start}, {midway}, {settled}"
            );
        }
        (seen.progress, seen.thumb)
    });
    // A fold reveals rows by the height they lay out at, so a row under it
    // keeps the sidebar's row height however long the sidebar grows. An ended
    // fold shows the branch's last row whole, right above that row.
    if matches!(phase, "settled" | "ended")
        && let Some((_, below, last)) = row_below(target)
    {
        let bounds = |id: &str| window.find(id.to_owned()).bounds();
        let row = bounds("nav:Workloads").size.height;
        assert_eq!(
            bounds(below).size.height,
            row,
            "{below} is not as high as a sidebar row"
        );
        // A fold builds only the rows it shows.
        let end = window
            .try_find(last.to_owned())
            .unwrap_or_else(|| panic!("{target} does not show {last} at {phase}"))
            .bounds();
        assert_eq!(end.size.height, row, "{last} is cut at {phase}");
        assert_eq!(
            end.bottom(),
            bounds(below).top(),
            "{target} is not fully open at {phase}"
        );
    }
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
        "settled" | "ended" => {
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
    if target == "chevron" {
        return None;
    }
    // A fold paints nothing of its own. The row under the branch shows it,
    // measured from the branch's row so that a scroll of the sidebar is left out.
    if let Some((branch, below, _)) = row_below(target) {
        let top = |id: &str| f32::from(window.find(id.to_owned()).bounds().origin.y);
        return Some(vec![(top(below) - top(branch)).to_string()]);
    }
    Some(
        quads(window, cx, target)
            .into_iter()
            .map(|quad| format!("{:?} {:?}", quad.bounds, quad.background))
            .collect(),
    )
}

/// The quads painted inside the target.
fn quads(
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
    target: &str,
) -> Vec<gpui_kit::Quad> {
    // A pill or a line slides from the option that was active to the chosen one.
    let (id, from): (ElementId, Option<&'static str>) = match target {
        "switch" => (ElementId::Name(format!("setting:{SETTING}").into()), None),
        "segment" => ("gitops-view:List".into(), Some("gitops-view:Cards")),
        "tab" => ("show-events".into(), Some("show-details")),
        "tab-actions" => ("show-actions".into(), Some("show-details")),
        _ => (transition(window, cx, target), None),
    };
    let mut area = window.find(id).bounds();
    if let Some(from) = from {
        area = area.union(&window.find(from).bounds());
    }
    // The tab's line sits 4 px under its trigger.
    let area = area.dilate(gpui_kit::px(4.)).scale(window.scale_factor());
    let edge = area.bottom_right();
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
        .collect()
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
        "tab-actions" => window.click("show-actions", cx),
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
        "branch" | "custom-group" => {
            let toggle = match target {
                "branch" => "tree-toggle:Workloads",
                _ => "custom-group:argoproj.io",
            };
            // The runner opens branches at once in every other scenario.
            Moving::set_instant_layout(false, cx);
            reveal(window, "sidebar-tree", toggle, cx);
            sidebar_live::click_sidebar(window, cx, toggle.into());
        }
        other => panic!("unknown motion target {other}"),
    }
    transition(window, cx, target)
}

/// A folding branch's row, the row under the branch, which moves as the
/// branch folds, and the branch's last row.
fn row_below(target: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match target {
        "branch" => Some(("nav:Workloads", "nav:Network", "tree:Workloads/CronJob")),
        "custom-group" => Some((
            "custom-group:argoproj.io",
            "custom-group:helm.toolkit.fluxcd.io",
            "custom-kind:argoproj.io/v1alpha1:ApplicationSet",
        )),
        _ => None,
    }
}

/// The id the target's transition reports under: the `eased` id, the sliding
/// indicator's id, the id of the element a `hover_fill` fills, or a fold's path.
fn transition(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, target: &str) -> ElementId {
    let id = match target {
        "switch" => format!("switch-thumb:{SETTING}"),
        "segment" => "segment-pill:gitops-view".into(),
        "tab" | "tab-actions" => "tab-line".into(),
        "row" => {
            let name = detail::resolve_resource(window, DEPLOYMENT, cx)
                .expect("the resource table shows the Deployment");
            format!("row:{name}")
        }
        "button" => "edit-draft".into(),
        "chevron" => "chevron:Workloads".into(),
        "branch" => "fold:Workloads".into(),
        "custom-group" => "fold:custom-group:argoproj.io".into(),
        other => panic!("unknown motion target {other}"),
    };
    ElementId::Name(id.into())
}
