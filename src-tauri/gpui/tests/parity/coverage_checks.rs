use super::*;
use gpui_kit::ElementId;
use gpui_kit::test::TestEventExt;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "coverageCheck" => {}
        _ => return false,
    }
    match string(step, "check") {
        "event-summary" => {
            assert_eq!(
                window.find("events-summary").label(),
                Some("Total 2 events; Warnings 1; Sources 1; Namespace payments")
            );
        }
        "session-action-failure" => {
            assert!(
                window
                    .find("session-action-message")
                    .label()
                    .unwrap()
                    .contains("Controlled stop failure")
            );
        }
        "import-skip" => {
            locate::click_label(window, "launcher-body", "Apply import", cx);
            window.render_frame(cx);
            window.find("settings-button:Apply import");
        }
        check @ ("rbac-user" | "rbac-group" | "rbac-sa") => {
            reveal(window, RBAC_TOP, "rbac-submitted", cx);
            let text = window.find("rbac-submitted").label().unwrap().to_owned();
            let expected: &[&str] = match check {
                "rbac-user" => &[
                    "{\"kind\":\"user\",\"username\":\"alice\",\"groups\":[\"developers\",\"system:authenticated\"]}",
                    "\"apiGroup\":\"apps\"",
                    "\"resource\":\"deployments\"",
                    "\"namespace\":\"payments\"",
                    "\"subresource\":\"scale\"",
                    "\"name\":\"payments-api\"",
                ],
                "rbac-group" => &[
                    "{\"kind\":\"group\",\"group\":\"developers\"}",
                    "\"kind\":\"nonResource\"",
                    "\"nonResourceUrl\":\"/healthz\"",
                ],
                _ => &[
                    "{\"kind\":\"serviceAccount\",\"name\":\"payments-api\",\"namespace\":\"payments\"}",
                    "\"verb\":\"delete\"",
                    "\"resource\":\"pods\"",
                ],
            };
            assert!(expected.iter().all(|value| text.contains(value)), "{text}");
            if check == "rbac-group" {
                assert!(!text.contains("\"resource\":"));
            }
        }
        "rbac-pages" => {
            let elements = window.elements();
            let labels: Vec<_> = elements.iter().filter(|element| matches!(element.path().last(), Some(ElementId::Name(name)) if name.starts_with("rbac-item:"))).map(|element| element.label().unwrap()).collect();
            assert_eq!(labels.len(), 51);
            assert_eq!(
                labels
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                labels.len()
            );
        }
        "helm-card-resources" | "helm-list-resources" => {
            assert!(seek(
                window,
                cx,
                "helm-list",
                "helm-resources:platform:metrics-gateway"
            ));
            if string(step, "check") == "helm-list-resources" {
                use gpui_kit::{InputEvent, ScrollDelta, ScrollEvent, point, px};
                let area = window.find("helm-list").bounds();
                let target = window
                    .find("helm-resources:platform:metrics-gateway")
                    .bounds();
                let right = area.right().min(window.viewport_size().width);
                window.dispatch_event(
                    ScrollEvent {
                        position: point(
                            area.left() + (right - area.left()) / 2.,
                            target.center().y,
                        ),
                        delta: ScrollDelta::Pixels(point(right - target.right(), px(0.))),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
                window.render_frame(cx);
            }
            window.click("helm-resources:platform:metrics-gateway", cx);
        }
        check @ ("emit-log" | "log-wrap" | "log-scroll-back" | "log-jump") => {
            super::log_coverage::check(window, cx, check)
        }
        "action-filter" => {
            window.find("action-row-replicas");
            assert!(window.try_find("action-row-rollout").is_none());
        }
        "replicas-reset" => {
            assert_eq!(window.find("action-replicas-input").value(), Some("2"));
            assert!(window.try_find("action-check").is_none());
        }
        "diff-compact" => {
            assert!(
                !window
                    .find("yaml-dry-run-diff")
                    .label()
                    .unwrap()
                    .contains("apiVersion: apps/v1")
            );
        }
        "palette-outside" => {
            let position = window.find("palette-dialog").bounds().origin / 2.;
            window.pointer_down(position, gpui_kit::MouseButton::Left, cx);
            window.pointer_up(position, gpui_kit::MouseButton::Left, cx);
        }
        "log-stacked" | "log-gutter" => {
            // log_row uses a 17px line, a bottom border, and 4px or 3px vertical padding.
            let expected = if string(step, "check") == "log-stacked" {
                26.
            } else {
                24.
            };
            assert_eq!(
                f32::from(window.find(("log-row", 0usize)).bounds().size.height),
                expected
            );
        }
        "log-one-line" => {
            window.click(("log-row", 0usize), cx);
            window.render_frame(cx);
            assert_eq!(window.find_all_by_label("Copy line").len(), 1);
            window.click("log-expand-all", cx);
            window.render_frame(cx);
            assert_eq!(window.find_all_by_label("Copy line").len(), 3);
            window.click("log-expand-all", cx);
            window.render_frame(cx);
            assert!(window.find_all_by_label("Copy line").is_empty());
            window.click(("log-row", 0usize), cx);
        }
        "map-groups" => {
            let nodes = |window: &gpui_kit::Window| {
                window.elements().iter().filter(|element| matches!(element.path().last(), Some(ElementId::Name(name)) if name.starts_with("map-node:"))).count()
            };
            let before = nodes(window);
            window.click("map-group:StorageClass", cx);
            window.render_frame(cx);
            assert!(nodes(window) > before);
            window.find("map-node:mock-dev:storage.k8s.io/v1:StorageClass::local-path");
            let group = window.find("map-group:StorageClass").bounds();
            let member = window
                .find("map-node:mock-dev:storage.k8s.io/v1:StorageClass::local-path")
                .bounds();
            assert!(member.top() > group.top());
            let position = gpui_kit::point(group.center().x, (group.top() + member.top()) / 2.);
            window.pointer_down(position, gpui_kit::MouseButton::Left, cx);
            window.pointer_up(position, gpui_kit::MouseButton::Left, cx);
            window.render_frame(cx);
            assert_eq!(nodes(window), before);
        }
        "map-group-expand" => {
            window.click("map-group:StorageClass", cx);
            window.render_frame(cx);
            window.find("map-node:mock-dev:storage.k8s.io/v1:StorageClass::local-path");
        }
        "map-group-collapsed" => {
            window.render_frame(cx);
            assert!(
                window
                    .try_find("map-node:mock-dev:storage.k8s.io/v1:StorageClass::local-path")
                    .is_none()
            );
        }
        "map-focused" => {
            reveal(window, "page", "map-viewport", cx);
            assert!(
                window
                    .find("map-counts")
                    .label()
                    .unwrap()
                    .contains("focused")
            );
            window.find("map-node:mock-dev:apps/v1:Deployment:payments:payments-api");
            window.find("map-node:mock-dev:apps/v1:ReplicaSet:payments:payments-api-7d9c9b7f8d");
            window.find("map-node:mock-dev:v1:Pod:payments:payments-api-7d9c9b7f8d-k2r9p");
            assert!(
                window
                    .try_find("map-node:mock-dev:apps/v1:Deployment:platform:metrics-gateway")
                    .is_none()
            );
        }
        check @ ("map-full" | "map-full-selection") => {
            reveal(window, "page", "map-viewport", cx);
            assert!(window.elements().iter().any(|element| element.visible() && matches!(element.path().last(), Some(ElementId::Name(name)) if name.starts_with("map-node:"))));
            assert!(
                !window
                    .find("map-counts")
                    .label()
                    .unwrap()
                    .contains("focused")
            );
            if check == "map-full-selection" {
                window.click("map-fit", cx);
                window.render_frame(cx);
                assert!(window.elements().iter().any(|element| element.visible() && matches!(element.path().last(), Some(ElementId::Name(name)) if name.starts_with("map-node:") && !name.contains(":payments:"))));
            }
        }
        "map-viewport" => {
            reveal(window, "page", "map-viewport", cx);
            let node = window.elements().iter().find(|element| matches!(element.path().last(), Some(ElementId::Name(name)) if name.starts_with("map-node:"))).map(locate::id).unwrap();
            let initial = window.find(node.clone()).bounds();
            window.click("map-zoom-in", cx);
            window.render_frame(cx);
            assert!(window.find(node.clone()).bounds().size.width > initial.size.width);
            let zoomed_width = window.find(node.clone()).bounds().size.width;
            window.click("map-zoom-out", cx);
            window.render_frame(cx);
            let before = window.find(node.clone()).bounds();
            assert!(before.size.width < zoomed_width);
            let bounds = window.find("map-viewport").bounds();
            let start = map_background(window, bounds);
            window.drag(
                start,
                gpui_kit::point(
                    (start.x + bounds.right()) / 2.,
                    (start.y + bounds.bottom()) / 2.,
                ),
                cx,
            );
            window.render_frame(cx);
            let after = window.find(node.clone()).bounds();
            assert!(after.origin.x > before.origin.x && after.origin.y > before.origin.y);
            window.click("map-fit", cx);
            window.render_frame(cx);
            let viewport = window.find("map-viewport").bounds();
            let nodes: Vec<_> = window.elements().into_iter().filter(|element| matches!(element.path().last(), Some(ElementId::Name(name)) if name.starts_with("map-node:"))).collect();
            let total = window
                .find("map-counts")
                .label()
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse::<usize>()
                .unwrap();
            assert_eq!(nodes.len(), total, "Fit omitted resources");
            assert!(
                nodes
                    .iter()
                    .all(|node| viewport.contains(&node.bounds().origin)
                        && viewport.contains(&node.bounds().bottom_right())),
                "Fit left a resource outside the viewport"
            );
        }
        "pagination" => {
            // Every row on the page, sorted, after checking that the group headers'
            // page counts add up to it. The table mounts only rows near its viewport,
            // so this scrolls through the page and keys each element by its path.
            let rows = |window: &mut gpui_kit::Window, cx: &mut gpui_kit::App| {
                let mut rows = std::collections::BTreeSet::new();
                let suffixes = [" resources on this page", " on this page"];
                let mut groups = suffixes.map(|_| std::collections::HashMap::new());
                super::detail::scan_table(window, cx, |window| {
                    for element in window.elements() {
                        if let Some(ElementId::Name(name)) = element.path().last()
                            && name.starts_with("resource:")
                        {
                            rows.insert(name.to_string());
                        }
                        for (suffix, counts) in suffixes.iter().zip(&mut groups) {
                            if let Some(count) = element
                                .label()
                                .and_then(|label| label.strip_suffix(suffix))
                                .and_then(|number| number.parse::<usize>().ok())
                            {
                                counts.insert(format!("{:?}", element.path()), count);
                            }
                        }
                    }
                    false
                });
                for counts in groups {
                    assert!(!counts.is_empty());
                    assert_eq!(counts.values().sum::<usize>(), rows.len());
                }
                rows.into_iter().collect::<Vec<_>>()
            };
            let first = rows(window, cx);
            let total = window
                .find("table-total")
                .label()
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse::<usize>()
                .unwrap();
            assert!(!first.is_empty());
            window.click("next-page", cx);
            window.render_frame(cx);
            let second = rows(window, cx);
            assert_eq!(first.len() + second.len(), total);
            assert_eq!(
                first
                    .iter()
                    .chain(second.iter())
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                total,
                "Pagination repeated a resource"
            );
            assert!(!second.is_empty());
            assert_ne!(first, second, "Next repeated page one");
            window.click("previous-page", cx);
            window.render_frame(cx);
            assert_eq!(first, rows(window, cx), "Previous did not restore page one");
        }
        "yaml-undo" => {
            let editor = locate::id(&window.find_by_label("Resource YAML"));
            window.within("detail-body").click(editor, cx);
            let original = window
                .find_by_label("Resource YAML")
                .value()
                .map(|value| value.to_string());
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-down"
                } else {
                    "ctrl-end"
                },
                cx,
            );
            window.input("undo: probe", cx);
            window.render_frame(cx);
            let edited = window
                .find_by_label("Resource YAML")
                .value()
                .map(|value| value.to_string());
            assert_ne!(edited, original, "The probe edit did not change the YAML");
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-z"
                } else {
                    "ctrl-z"
                },
                cx,
            );
            window.render_frame(cx);
            assert_eq!(
                window
                    .find_by_label("Resource YAML")
                    .value()
                    .map(|value| value.to_string()),
                original,
                "Undo did not restore the original YAML"
            );
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-shift-z"
                } else {
                    "ctrl-y"
                },
                cx,
            );
            window.render_frame(cx);
            assert_eq!(
                window
                    .find_by_label("Resource YAML")
                    .value()
                    .map(|value| value.to_string()),
                edited,
                "Redo did not restore the edited YAML"
            );
        }
        "yaml-editor" => {
            let editor = locate::id(&window.find_by_label("Resource YAML"));
            window.within("detail-body").click(editor, cx);
            window.press("secondary-a", cx);
            window.input("first: one\nsecond: two\n", cx);
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-up"
                } else {
                    "ctrl-home"
                },
                cx,
            );
            window.press("shift-end", cx);
            window.input("first: replaced", cx);
            window.render_frame(cx);
            assert_eq!(
                window.find_by_label("Resource YAML").value(),
                Some("first: replaced\nsecond: two\n")
            );
            window.press(
                if cfg!(target_os = "macos") {
                    "cmd-down"
                } else {
                    "ctrl-end"
                },
                cx,
            );
            window.input("third: three", cx);
            window.render_frame(cx);
            assert_eq!(
                window.find_by_label("Resource YAML").value(),
                Some("first: replaced\nsecond: two\nthird: three")
            );
        }
        "timeline" => {
            assert_eq!(
                window.find("detail-incident-summary").label(),
                Some("Incident summary for Deployment/payments-api")
            );
            let evidence = |window: &gpui_kit::Window| {
                window
                    .elements()
                    .iter()
                    .filter(|element| element.label() == Some("Timeline evidence"))
                    .count()
            };
            reveal(window, "detail-body", "timeline-expand-all", cx);
            window.click("timeline-expand-all", cx);
            window.render_frame(cx);
            let expanded = evidence(window);
            assert!(expanded > 0);
            window.click("timeline-collapse-all", cx);
            window.render_frame(cx);
            assert_eq!(evidence(window), 0);
            let first = window.elements().iter().find(|element| matches!(element.path().last(), Some(ElementId::Name(name)) if name.starts_with("timeline-entry:"))).map(locate::id).unwrap();
            locate::click_in(window, "detail-body", first.clone(), cx);
            window.render_frame(cx);
            assert_eq!(evidence(window), 1);
            window.click(first, cx);
            window.render_frame(cx);
            assert_eq!(evidence(window), 0);
        }
        "helm-resources" => {
            assert!(seek(window, cx, "helm-list", "helm-details-resources"));
            window.click("helm-details-resources", cx);
        }
        other => panic!("unknown coverage check {other}"),
    }
    true
}

fn map_background(
    window: &gpui_kit::Window,
    viewport: gpui_kit::Bounds<gpui_kit::Pixels>,
) -> gpui_kit::Point<gpui_kit::Pixels> {
    let y = viewport.center().y;
    let mut occupied: Vec<_> = window
        .elements()
        .iter()
        .filter_map(|element| {
            let Some(ElementId::Name(name)) = element.path().last() else {
                return None;
            };
            let bounds = element.bounds();
            if !(name.starts_with("map-node:")
                || name.starts_with("map-group:")
                || matches!(name.as_ref(), "map-zoom-in" | "map-zoom-out" | "map-fit"))
                || bounds.top() > y
                || bounds.bottom() < y
            {
                return None;
            }
            let left = bounds.left().max(viewport.left());
            let right = bounds.right().min(viewport.right());
            (left < right).then_some((left, right))
        })
        .collect();
    occupied.sort_by_key(|(left, _)| *left);
    let mut left = viewport.left();
    for (start, end) in occupied {
        if start > left {
            return gpui_kit::point((left + start) / 2., y);
        }
        left = left.max(end);
    }
    assert!(
        left < viewport.right(),
        "No empty canvas on the map center line"
    );
    gpui_kit::point((left + viewport.right()) / 2., y)
}
