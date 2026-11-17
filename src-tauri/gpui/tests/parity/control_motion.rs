//! `motion:<target>:<phase>` checks: a control moves to its new state over
//! `control_motion::DURATION` instead of jumping. `start` changes the control's state and
//! expects the transition at one end; `midway` expects it between the ends after
//! the scenario advanced the clock by half the duration; `settled` expects it over.
//! Mirrors `e2e/parity/web-control-motion.ts`.
use super::*;
use gpui_kit::ElementId;
use gpui_kit::test::TestEventExt;
use kubecove_gpui::control_motion::Moving;
use std::cell::Cell;

const SETTING: &str = "Show exact timestamps";
const DEPLOYMENT: &str = "resource:Deployment:payments:payments-api";

thread_local! {
    /// The progress the last `start` check saw, and the switch thumb's left edge then.
    static START: Cell<Option<(f32, f32)>> = const { Cell::new(None) };
}

pub(super) fn handles(check: &str) -> bool {
    check.starts_with("motion:")
}

pub(super) fn check(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, check: &str) {
    let mut parts = check.split(':').skip(1);
    let (target, phase) = (parts.next().unwrap(), parts.next().unwrap());
    if phase == "start" {
        let moving = change(window, cx, target);
        window.render_frame(cx);
        let progress = Moving::progress(&moving, cx)
            .unwrap_or_else(|| panic!("{target} changed without a transition"));
        assert!(
            progress == 0. || progress == 1.,
            "{target} jumped to {progress}"
        );
        START.set(Some((progress, thumb(window, target))));
        return;
    }
    let moving = transition(window, cx, target);
    let (start, thumb_start) = START.get().expect("a start check ran first");
    let progress = Moving::progress(&moving, cx);
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
        other => panic!("unknown motion target {other}"),
    }
    transition(window, cx, target)
}

/// The id the target's transition reports under: the `eased` id, or the id of
/// the element a `hover_fill` fills.
fn transition(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, target: &str) -> ElementId {
    let id = match target {
        "switch" => format!("switch-thumb:{SETTING}"),
        "segment" => "segment:gitops-view:List".into(),
        "tab" => "tab:show-events".into(),
        "row" => {
            let name = detail::resolve_resource(window, DEPLOYMENT, cx)
                .expect("the resource table shows the Deployment");
            format!("row:{name}")
        }
        "button" => "edit-draft".into(),
        "chevron" => "chevron:Workloads".into(),
        other => panic!("unknown motion target {other}"),
    };
    ElementId::Name(id.into())
}
