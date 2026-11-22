//! Lookups for the workspace-surface steps, whose targets are found by their
//! accessible label inside a scope instead of a fixed element ID.

use gpui_kit::test::{ElementSnapshot, TestEventExt, TestQueryExt, TestWindowExt};
use gpui_kit::{App, ElementId, InputEvent, ScrollDelta, ScrollEvent, Window, point, px};

/// The one element labeled `label` inside `scope` whose description matches,
/// such as a palette resource described by its kind and namespace.
pub fn described(
    window: &mut Window,
    scope: &'static str,
    label: &str,
    description: &str,
) -> Option<ElementSnapshot> {
    let mut found: Vec<ElementSnapshot> = window
        .within(scope)
        .find_all_by_label(label)
        .into_iter()
        .filter(|element| element.description() == Some(description))
        .collect();
    assert!(
        found.len() <= 1,
        "ambiguous {label:?} {description:?} in {scope}"
    );
    found.pop()
}

/// The element's own ID, the last segment of its path.
pub fn id(element: &ElementSnapshot) -> ElementId {
    element
        .path()
        .last()
        .expect("observed element has an ID")
        .clone()
}

/// The mounted rows of a virtualized list as `(list position, label)`, top to
/// bottom. A row keeps its list position in its `(name, position)` ID, so after
/// a scroll the first mounted row need not be position 0.
pub fn mounted_rows(window: &Window, name: &str) -> Vec<(usize, String)> {
    let mut rows: Vec<(usize, String)> = window
        .elements()
        .iter()
        .filter_map(|element| match element.path().last() {
            Some(ElementId::NamedInteger(id, position)) if id.as_ref() == name => Some((
                usize::try_from(*position).expect("list position fits usize"),
                element.label().unwrap_or_default().to_owned(),
            )),
            _ => None,
        })
        .collect();
    rows.sort_by_key(|(position, _)| *position);
    rows.dedup_by_key(|(position, _)| *position);
    rows
}

/// Scrolls `area`, a nested scroll area, until `target` sits at its middle,
/// then clicks it, as Playwright scrolls a target into view before a click.
pub fn click_in(window: &mut Window, area: &'static str, target: ElementId, cx: &mut App) {
    window.render_frame(cx);
    let element = window.find(target.clone());
    if !element.visible() {
        let bounds = window.find(area).bounds();
        let position = bounds.origin + point(px(4.), px(4.));
        let delta = bounds.center().y - element.bounds().center().y;
        window.pointer_move(position, None, cx);
        window.dispatch_event(
            ScrollEvent {
                position,
                delta: ScrollDelta::Pixels(point(px(0.), delta)),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }
    window.click(target, cx);
}

/// Clicks the one element labeled `label` inside `area`.
pub fn click_label(window: &mut Window, area: &'static str, label: &str, cx: &mut App) {
    let target = id(&window.within(area).find_by_label(label));
    click_in(window, area, target, cx);
}

/// Keyboard focus for an observed element: `Some(true)` when focused, `Some(false)` when not,
/// and `None` when the element has no observed focus binding.
///
/// `ElementSnapshot::focused` panics on the binding-missed state, which the sidebar navigation
/// reaches because it advertises focus without a tracked handle. Scenarios query focus across the
/// whole tree, so this reads that exact state from the snapshot and reports it as `None`.
pub fn focus_state(element: &ElementSnapshot) -> Option<bool> {
    if format!("{element:?}").contains("focused: <binding missed>") {
        return None;
    }
    element.focused()
}
