use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "cancelYamlEdit" => window.click("discard-draft", cx),
        "openDocumentation" => window.click("open-documentation", cx),
        "expectDocumentationRequest" => {}
        "revealSecretValue" | "hideSecretValue" => {
            reveal(window, "detail-body", "secret-reveal:data:token", cx);
            window.click("secret-reveal:data:token", cx);
        }
        "expectSecretValue" => {
            assert_eq!(
                window.try_find("secret-value:data:token").is_some(),
                step["visible"].as_bool().unwrap()
            );
            if step["visible"] == true {
                assert_eq!(
                    window.find("secret-value:data:token").label(),
                    Some("secret")
                );
            }
        }
        "expectYamlLens" => {
            let line = number(step, "line");
            if line == 0 {
                assert!(window.try_find("yaml-lens").is_none());
            } else {
                assert_eq!(
                    window.find("yaml-lens").label(),
                    Some(format!("YAML error at line {line}").as_str())
                );
            }
        }
        "refreshLatencyReport" | "clearLatencyReport" => {
            let id = if step["action"] == "refreshLatencyReport" {
                "latency-refresh"
            } else {
                "latency-clear"
            };
            reveal(window, "settings-heading", id, cx);
            window.click(id, cx);
        }
        "expectLatencyEmpty" => assert!(window.try_find("latency-empty").is_some()),
        "expectLatencyTiming" => {
            let id = format!("latency-command:{}", string(step, "command"));
            reveal(window, "settings-heading", &id, cx);
            let label = window.find(id).label().unwrap().to_owned();
            assert!(label.contains("ms"));
        }
        _ => return false,
    }
    true
}
