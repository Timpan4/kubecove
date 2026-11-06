use gpui_kit::test::TestWindowExt;
use serde_json::Value;

pub(super) fn apply(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    let s = |key: &str| step[key].as_str().expect("string field");
    match s("action") {
        "expectTopologyDenied" => {
            let error = window.find("map-error");
            assert!(error.visible());
            assert_eq!(
                error.label(),
                Some("KubeCove does not have permission for this")
            );
        }
        "expectUsageFooter" => assert_eq!(window.find("usage-footer").label(), Some(s("text"))),
        "openUsageTree" => window.click("usage-footer", cx),
        "expectUsageProcess" => assert!(
            window
                .find(format!("usage-process:{}", s("label")))
                .visible()
        ),
        "expectNoUsageFooter" => assert!(window.try_find("usage-footer").is_none()),
        "selectNamespace" => {
            let id = format!("namespace:{}", s("namespace"));
            super::reveal(window, "sidebar-tree", &id, cx);
            window.click(id, cx);
        }
        "setTopologyMode" => window.click(format!("map-mode:{}", s("mode")), cx),
        "expectTopology" => assert_eq!(
            window.find("map-counts").label(),
            Some(format!("{} nodes / {} edges", step["nodes"], step["edges"]).as_str())
        ),
        "expectTopologyEdge" => assert!(
            window
                .try_find(format!("map-edge:{}->{}", s("source"), s("target")))
                .is_some()
        ),
        "selectTopologyNode" => window.click(format!("map-node:{}", s("id")), cx),
        "expectTopologyEmpty" => assert_eq!(window.find("map-empty").label(), Some(s("title"))),
        "expectNoTopologyNode" => {
            assert!(window.try_find(format!("map-node:{}", s("id"))).is_none())
        }
        "expectResourceMetrics" => {
            assert!(super::reveal_resource(window, &super::row(step), cx));
            for (column, field) in [("CPU", "cpu"), ("Memory", "memory")] {
                let id = format!(
                    "metric:{}:{}:{}:{column}",
                    s("kind"),
                    s("namespace"),
                    s("name")
                );
                assert_eq!(
                    window.find(id).label(),
                    Some(format!("{column} usage {}", s(field)).as_str())
                );
            }
        }
        _ => return false,
    }
    true
}
