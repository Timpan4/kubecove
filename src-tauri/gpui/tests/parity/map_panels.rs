use gpui_kit::test::TestWindowExt;
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
            assert_eq!(
                window.find(format!("map-node:{}", s("id"))).focused(),
                Some(true)
            );
        }
        "expectNoMapFocus" => assert_ne!(
            window.find(format!("map-node:{}", s("id"))).focused(),
            Some(true)
        ),
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
