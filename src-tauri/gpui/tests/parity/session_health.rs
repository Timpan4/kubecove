use super::*;

/// Header live-session popover, health badge tooltips and timestamp tooltips.
/// Hover text is the element's description, so each check compares the text the
/// web tooltip shows with the same text on the native element.
pub fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "openLiveSessionPopover" => {
            if window.try_find("live-session-popover").is_none() {
                window.click("live-sessions", cx);
            }
        }
        "expectLiveSessionRow" => {
            let target = string(step, "target");
            assert_eq!(
                window.find(format!("live-session-status:{target}")).label(),
                Some(string(step, "status"))
            );
            for control in strings(step, "controls") {
                let id = format!("live-session:{target}:{}", control.to_lowercase());
                assert!(window.try_find(id.clone()).is_some(), "missing {id}");
            }
        }
        "liveSessionControl" => match string(step, "operation") {
            "Manage" => window.click("live-sessions-manage", cx),
            operation => {
                let id = format!(
                    "live-session:{}:{}",
                    string(step, "target"),
                    operation.to_lowercase()
                );
                locate::click_in(window, "live-session-list", id.into(), cx)
            }
        },
        "expectHealthTooltip" => {
            let key = identity(step);
            let tooltip = window
                .find(format!("health:{key}"))
                .description()
                .map(str::to_owned)
                .unwrap_or_default();
            assert!(
                tooltip.starts_with(string(step, "state")),
                "health tooltip was {tooltip:?}"
            );
            assert!(tooltip.contains(string(step, "source")), "{tooltip:?}");
            for status in strings(step, "statuses") {
                assert!(tooltip.contains(&status), "{tooltip:?}");
            }
            assert_eq!(
                window.try_find(format!("partial:{key}")).is_some(),
                step["partial"].as_bool().unwrap_or(false)
            );
        }
        "expectTimestampTooltip" => {
            assert_eq!(
                window.find(format!("age:{}", identity(step))).description(),
                Some(string(step, "value"))
            );
        }
        "expectEventTimeTooltip" => {
            let tooltip = window
                .find(format!("event-time:{}", number(step, "index")))
                .description()
                .map(str::to_owned)
                .unwrap_or_default();
            assert!(is_exact_timestamp(&tooltip), "event time was {tooltip:?}");
        }
        _ => return false,
    }
    true
}

/// `kind:namespace:name`, the key native rows use in their test IDs.
fn identity(step: &Value) -> String {
    format!(
        "{}:{}:{}",
        string(step, "kind"),
        string(step, "namespace"),
        string(step, "name")
    )
}

/// `YYYY-MM-DD HH:MM:SS.mmm`, with ` UTC` when the preference is UTC.
fn is_exact_timestamp(text: &str) -> bool {
    let body = text.strip_suffix(" UTC").unwrap_or(text);
    body.len() == 23
        && body.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            10 => byte == b' ',
            13 | 16 => byte == b':',
            19 => byte == b'.',
            _ => byte.is_ascii_digit(),
        })
}
