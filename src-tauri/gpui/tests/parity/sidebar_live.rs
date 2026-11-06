use super::*;
use gpui_kit::InputEvent;
use gpui_kit::px;
use gpui_kit::test::TestEventExt;

/// The test id of a sidebar row from its path: `nav:` for top-level built-in
/// groups, `namespace:` for namespaces and `tree:` for nested rows.
pub(super) fn sidebar_row_id(path: &[&str]) -> String {
    let joined = path.join("/");
    let builtin = [
        "Workloads",
        "Network",
        "Config",
        "Storage",
        "RBAC",
        "Cluster Overview",
    ]
    .contains(&path[0]);
    if path.len() > 1 {
        format!("tree:{joined}")
    } else if builtin {
        format!("nav:{joined}")
    } else {
        format!("namespace:{joined}")
    }
}

pub(super) fn click_sidebar(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, id: String) {
    let position = window.find(id).bounds().center();
    window.pointer_move(position, None, cx);
    window.dispatch_event(
        gpui_kit::MouseDownEvent {
            button: gpui_kit::MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    window.draw(cx).clear(cx);
    window.dispatch_event(
        gpui_kit::MouseUpEvent {
            button: gpui_kit::MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
    let start = std::time::Instant::now();
    window.draw(cx).clear(cx);
    println!(
        "sidebar_event_frame_ms={:.6}",
        start.elapsed().as_secs_f64() * 1000.
    );
}

/// The description of a row's label while its collapsed branch holds the selection.
const CONTAINS_SELECTION: &str = "Contains the selected item";

/// Wheels the sidebar tree to its top or bottom, one half viewport at a time,
/// until a wheel step no longer changes the rendered scene.
fn scroll_sidebar(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, bottom: bool) {
    let position = window.find("sidebar-tree").bounds().center();
    let direction = if bottom { -1. } else { 1. };
    let step = f32::from(window.viewport_size().height) / 2. * direction;
    window.pointer_move(position, None, cx);
    loop {
        window.render_frame(cx);
        let before = scene(window);
        window.dispatch_event(
            gpui_kit::ScrollEvent {
                position,
                delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(step))),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
        if scene(window) == before {
            break;
        }
    }
}

/// Every rendered element and its bounds. A scroll changes at least one.
fn scene(window: &gpui_kit::Window) -> Vec<(String, gpui_kit::Bounds<gpui_kit::Pixels>)> {
    window
        .elements()
        .iter()
        .map(|element| (format!("{:?}", element.path()), element.bounds()))
        .collect()
}

pub(super) fn apply(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "scrollSidebar" => scroll_sidebar(window, cx, string(step, "to") == "bottom"),
        "expectSidebarReveal" => {
            let path: Vec<_> = step["path"]
                .as_array()
                .unwrap()
                .iter()
                .map(|part| part.as_str().unwrap())
                .collect();
            let id = sidebar_row_id(&path);
            // The reveal scrolls on the frame after its probe paints, so let
            // one more frame settle before reading bounds.
            window.render_frame(cx);
            window.render_frame(cx);
            let viewport = window.find("sidebar-tree").bounds();
            let visible = window.try_find(id.clone()).is_some_and(|row| {
                row.bounds().top() >= viewport.top() && row.bounds().bottom() <= viewport.bottom()
            });
            let highlighted = window
                .try_find(format!("label:{id}"))
                .is_some_and(|label| label.description() == Some(CONTAINS_SELECTION));
            assert_eq!(visible, step["visible"] == true, "{step}");
            assert_eq!(highlighted, step["highlighted"] == true, "{step}");
        }
        "sidebarToggle" | "sidebarSelect" | "expectSidebarRow" => {
            let path: Vec<_> = step["path"]
                .as_array()
                .unwrap()
                .iter()
                .map(|part| part.as_str().unwrap())
                .collect();
            let joined = path.join("/");
            let id = sidebar_row_id(&path);
            match string(step, "action") {
                "expectSidebarRow" => {
                    let found = window.try_find(id);
                    assert_eq!(found.is_some(), step["visible"] == true, "{step}");
                    if let Some(row) = found {
                        assert_eq!(
                            row.description(),
                            Some(if step["selected"] == true {
                                "selected"
                            } else {
                                "unselected"
                            }),
                            "{step}"
                        );
                    }
                }
                action => {
                    let id = if action == "sidebarToggle" {
                        format!("tree-toggle:{joined}")
                    } else {
                        id
                    };
                    reveal(window, "sidebar-tree", &id, cx);
                    click_sidebar(window, cx, id);
                }
            }
        }
        "collapseSidebar" => window.click("collapse-sidebar", cx),
        "expectSidebarBranch" => {
            let page = string(step, "page");
            let (id, child) = match page {
                "GitOps" => ("nav:GitOps", "nav:Argo CD"),
                "Argo CD" => ("nav:Argo CD", "nav:Argo CD Applications"),
                "Custom Resources" => ("nav:Custom Resources", "custom-resource-search"),
                "Helm" => ("nav:Helm", "nav:Helm/Releases"),
                "testing.example.io" => (
                    "custom-group:testing.example.io",
                    "custom-kind:testing.example.io/v1:Widget",
                ),
                _ => panic!("unknown sidebar branch: {page}"),
            };
            window.find(id);
            assert_eq!(window.try_find(child).is_some(), step["expanded"] == true);
        }
        "refreshPage" => window.click(
            if step["page"] == "RBAC" {
                "rbac-refresh"
            } else {
                "incidents-refresh"
            },
            cx,
        ),
        "replayPageReads" | "focusStalePage" => {}
        "expectPageText" => {
            if string(step, "text") == "Scripted page refresh failure" {
                let title = [
                    "Helm refresh failed",
                    "RBAC refresh failed",
                    "Incident refresh failed",
                    "GitOps refresh failed",
                ]
                .into_iter()
                .find(|title| window.try_find(*title).is_some())
                .expect("refresh failure panel");
                window.click(format!("{title}:detail"), cx);
                window.render_frame(cx);
                assert_eq!(
                    window.find(format!("{title}:technical")).label(),
                    Some(string(step, "text"))
                );
            } else {
                window.find_by_label(string(step, "text"));
            }
        }
        "searchCustomKinds" => {
            if window.try_find("custom-resource-search").is_none() {
                reveal(window, "sidebar-tree", "nav:Custom Resources", cx);
                window.click("nav:Custom Resources", cx);
                window.render_frame(cx);
            }
            replace_text(window, cx, "custom-resource-search", string(step, "value"));
        }
        "searchKindPicker" => replace_text(window, cx, "kind-search", string(step, "value")),
        "expectCustomGroup" => assert_eq!(
            window
                .try_find(format!("custom-group:{}", string(step, "group")))
                .is_some(),
            step["visible"] == true
        ),
        "openKindPicker" => window.click("kinds-picker", cx),
        "closeKindPicker" => {
            if window.try_find("kind-options").is_some() {
                window.click("kinds-picker", cx);
            }
        }
        "selectAllKinds" => window.click("select-all-kinds", cx),
        "toggleKind" | "expectKindChecked" => {
            let option = locate::described(
                window,
                "kind-options",
                string(step, "kind"),
                if step["checked"] == true {
                    "checked"
                } else {
                    "unchecked"
                },
            );
            if step["action"] == "expectKindChecked" {
                assert!(option.is_some(), "wrong kind selection: {step}");
            } else if option.is_none() {
                let found = window
                    .within("kind-options")
                    .find_by_label(string(step, "kind"));
                locate::click_in(window, "kind-options", locate::id(&found), cx);
            }
        }
        _ => return false,
    }
    true
}
