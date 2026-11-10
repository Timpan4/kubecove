use gpui_kit::test::{TestQueryExt, TestWindowExt};
use serde_json::Value;

/// Map and table collapse, graph focus and the detail splitter. Mirrors `e2e/parity/web-panels.ts`.
pub(super) fn apply(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    let s = |key: &str| step[key].as_str().expect("string field");
    match s("action") {
        "panelControl" => window.click(format!("panel-control:{}", s("control")), cx),
        "expectPanelControl" => {
            let present = window
                .try_find(format!("panel-control:{}", s("control")))
                .is_some();
            assert_eq!(present, step["present"].as_bool().expect("boolean field"));
        }
        "skipGraph" => window.click("map-skip-graph", cx),
        "mapKey" => window.press(&s("key").replace('+', "-").to_lowercase(), cx),
        "expectMapFocus" => {
            let focused: Vec<String> = window
                .elements()
                .iter()
                .filter(|element| super::locate::focus_state(element) == Some(true))
                .map(|element| format!("{:?}", element.path().last()))
                .collect();
            assert_eq!(
                window.find(format!("map-node:{}", s("id"))).focused(),
                Some(true),
                "focused elements in the window: {focused:?}"
            );
        }
        "expectNoMapFocus" => assert_ne!(
            window.find(format!("map-node:{}", s("id"))).focused(),
            Some(true)
        ),
        "panMap" => {
            // Drag from the viewport center by the same offset the web driver uses.
            let viewport = window.find("map-viewport").bounds();
            let from = gpui_kit::point(
                viewport.origin.x + viewport.size.width / 2.,
                viewport.origin.y + viewport.size.height / 2.,
            );
            let to = gpui_kit::point(
                from.x + gpui_kit::px(step["dx"].as_f64().expect("number field") as f32),
                from.y + gpui_kit::px(step["dy"].as_f64().expect("number field") as f32),
            );
            window.drag(from, to, cx);
        }
        "expectMapNodeInView" => {
            let node = window.find(format!("map-node:{}", s("id"))).bounds();
            let viewport = window.find("map-viewport").bounds();
            let overlaps = f32::from(node.origin.x)
                < f32::from(viewport.origin.x + viewport.size.width)
                && f32::from(node.origin.x + node.size.width) > f32::from(viewport.origin.x)
                && f32::from(node.origin.y) < f32::from(viewport.origin.y + viewport.size.height)
                && f32::from(node.origin.y + node.size.height) > f32::from(viewport.origin.y);
            assert_eq!(overlaps, step["inView"].as_bool().expect("boolean field"));
        }
        "clickMapControl" => window.click(map_control_id(s("control")), cx),
        "expectMapControlFocus" => {
            assert_eq!(
                window.find(map_control_id(s("control"))).focused(),
                Some(true)
            );
        }
        "expectMapZoom" => {
            let expected = format!("{:.2}", step["zoom"].as_f64().expect("number field"));
            assert_eq!(window.find("map-viewport").value(), Some(expected.as_str()));
        }
        "resizeDetail" => {
            window.click("detail-resize", cx);
            window.press(&s("key").replace("Arrow", "").to_lowercase(), cx);
        }
        "expectDetailResize" => {
            let expected = step["value"].to_string();
            assert_eq!(
                window.find("detail-resize").value(),
                Some(expected.as_str())
            );
        }
        _ => return false,
    }
    true
}

/// The test id of a zoom control, named by its xyflow label.
fn map_control_id(label: &str) -> &'static str {
    match label {
        "Zoom In" => "map-zoom-in",
        "Zoom Out" => "map-zoom-out",
        "Fit View" => "map-fit",
        other => panic!("unknown map control: {other}"),
    }
}
