use super::*;
pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "overviewHealth" => locate::click_in(
            window,
            "page",
            format!("overview-health:{}", string(step, "filter")).into(),
            cx,
        ),
        "overviewNamespace" => locate::click_in(
            window,
            "page",
            format!("overview-namespace:{}", string(step, "namespace")).into(),
            cx,
        ),
        "expectOverviewProblem"
        | "openOverviewProblem"
        | "toggleOverviewProblem"
        | "expectNoOverviewProblem" => {
            let key = format!(
                "{}:{}:{}:{}",
                string(step, "kind"),
                string(step, "namespace"),
                string(step, "name"),
                step["context"].as_str().unwrap_or("mock-dev")
            );
            if step["action"] == "openOverviewProblem" {
                locate::click_in(window, "page", format!("overview-open:{key}").into(), cx);
            } else if step["action"] == "toggleOverviewProblem" {
                let target = format!("overview-expand:{key}");
                reveal(window, "page", &target, cx);
                window.click(target, cx);
            } else if step["action"] == "expectNoOverviewProblem" {
                assert!(window.try_find(format!("overview-problem:{key}")).is_none());
            } else {
                let id = format!("overview-problem:{key}");
                assert!(seek(window, cx, "page", &id));
                assert_eq!(
                    window.find(id).description(),
                    Some(format!("{}|{}", string(step, "status"), string(step, "ready")).as_str())
                );
            }
        }
        _ => return false,
    }
    true
}
