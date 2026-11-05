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
            // One lens per marked line, as the web shows them.
            let line = number(step, "line");
            let lenses = window.find_all("yaml-lens");
            if line == 0 {
                assert!(lenses.is_empty());
            } else {
                let label = format!("YAML error at line {line}");
                assert!(
                    lenses
                        .iter()
                        .any(|lens| lens.label() == Some(label.as_str())),
                    "no lens labelled {label:?}"
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
