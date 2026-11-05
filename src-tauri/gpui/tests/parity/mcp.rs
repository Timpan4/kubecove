use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "fillMcpPort" => replace_text(window, cx, "mcp-port", string(step, "value")),
        "mcpControl" => {
            if step["control"] == "Submit port" {
                window.press("enter", cx);
            } else {
                window.click(format!("mcp-control:{}", string(step, "control")), cx);
            }
        }
        "expectMcpRequests" => {}
        "expectMcpState" => {
            assert_eq!(
                window.find("mcp-control:MCP server").checked(),
                step["enabled"].as_bool()
            );
            assert_eq!(window.find("mcp-port").value(), Some(string(step, "port")));
            assert_eq!(
                window.find("mcp-control:Save").description(),
                Some(if step["save"] == true {
                    "enabled"
                } else {
                    "disabled"
                })
            );
            assert_eq!(
                window
                    .find("mcp-control:Copy Claude Code command")
                    .description(),
                Some(if step["command"] == true {
                    "enabled"
                } else {
                    "disabled"
                })
            );
            if step["running"] == true {
                assert_eq!(
                    window.find("setting-row:MCP server").label(),
                    Some(
                        format!(
                            "Listening at http://127.0.0.1:{}/mcp with 11 read-only tools.",
                            string(step, "port")
                        )
                        .as_str()
                    )
                );
            } else {
                assert!(
                    !window
                        .find("setting-row:MCP server")
                        .label()
                        .unwrap()
                        .starts_with("Listening at")
                );
            }
        }
        "expectMcpCopy" => {
            let token = if step["rotated"] == true {
                "parity-mcp-rotated-credential"
            } else {
                "parity-mcp-credential"
            };
            let expected = if step["kind"] == "token" {
                token.to_owned()
            } else {
                format!(
                    "claude mcp add --transport http kubecove http://127.0.0.1:47615/mcp --header \"Authorization: Bearer {token}\""
                )
            };
            assert!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref()
                    == Some(&expected),
                "Wrong MCP clipboard content"
            );
            assert_eq!(
                window.find("mcp-announcement").label(),
                Some(if step["kind"] == "token" {
                    "Copied token."
                } else {
                    "Copied Claude Code command."
                })
            );
            assert!(
                !window.elements().iter().any(|element| element
                    .label()
                    .is_some_and(|label| label.contains(token))
                    || element.value().is_some_and(|value| value.contains(token))),
                "Credential must not enter rendered labels or values"
            );
        }
        _ => return false,
    }
    true
}
