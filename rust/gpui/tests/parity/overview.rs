use super::*;
pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "coverageClick" => {
            let label = string(step, "label");
            let id = match label {
                "Next findings" => "overview-next".into(),
                "Previous findings" => "overview-previous".into(),
                "Next applications" => "overview-applications-next".into(),
                "Previous applications" => "overview-applications-previous".into(),
                _ if label.starts_with("Inspect Application ") => {
                    locate::id(&window.find_by_label(label))
                }
                _ => return false,
            };
            locate::click_in(window, "page", id, cx);
        }
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
            let label = format!(
                "{} / {} in {} on {}",
                string(step, "kind"),
                string(step, "name"),
                string(step, "namespace"),
                step["context"].as_str().unwrap_or("mock-dev")
            );
            let mut matching = window.find_all_by_label(&label).into_iter().filter(|row| {
                let Some(api) = step["apiVersion"].as_str() else {
                    return true;
                };
                let gpui_kit::ElementId::Name(id) = locate::id(row) else {
                    return false;
                };
                let Some(key) = id.strip_prefix("overview-problem:") else {
                    return false;
                };
                serde_json::from_str::<Value>(key).expect("resource identity JSON")[3] == api
            });
            let row = matching.next();
            assert!(
                matching.next().is_none(),
                "finding identity is ambiguous; specify apiVersion"
            );
            if step["action"] == "expectNoOverviewProblem" {
                assert!(row.is_none());
                return true;
            }
            let row = row.expect("matching finding identity");
            let gpui_kit::ElementId::Name(id) = locate::id(&row) else {
                panic!("finding has a named identity")
            };
            let key = id
                .strip_prefix("overview-problem:")
                .expect("finding row identity");
            if let Some(activation) = step["key"].as_str() {
                use gpui_kit::InputEvent;
                let prefix = if step["action"] == "openOverviewProblem" {
                    "overview-open"
                } else {
                    "overview-expand"
                };
                let target = format!("{prefix}:{key}");
                reveal(window, "page", &target, cx);
                for _ in 0..window.elements().len() {
                    if window.find(target.clone()).focused() == Some(true) {
                        break;
                    }
                    window.press("tab", cx);
                    window.render_frame(cx);
                }
                assert_eq!(window.find(target).focused(), Some(true));
                let keystroke = match activation {
                    "enter" => gpui_kit::Keystroke::parse("enter"),
                    "space" => gpui_kit::Keystroke::parse("space"),
                    _ => panic!("activation key was validated by the shared contract"),
                }
                .unwrap();
                window.dispatch_event(
                    gpui_kit::KeyDownEvent {
                        keystroke: keystroke.clone(),
                        is_held: false,
                        prefer_character_input: false,
                    }
                    .to_platform_input(),
                    cx,
                );
                window.dispatch_event(gpui_kit::KeyUpEvent { keystroke }.to_platform_input(), cx);
            } else if step["action"] == "openOverviewProblem" {
                locate::click_in(window, "page", format!("overview-open:{key}").into(), cx);
            } else if step["action"] == "toggleOverviewProblem" {
                let target = format!("overview-expand:{key}");
                reveal(window, "page", &target, cx);
                window.click(target, cx);
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
