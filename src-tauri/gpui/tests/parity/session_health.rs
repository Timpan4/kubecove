use super::*;
use gpui_kit::ElementId;

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
            let row = session_row(window, step);
            let status = string(step, "status");
            let scope = window.within(row.clone());
            assert!(
                scope.try_find_by_label(status).is_some(),
                "{row:?} is not {status}"
            );
            for control in strings(step, "controls") {
                assert!(
                    scope.try_find_by_label(&control).is_some(),
                    "{row:?} has no {control}"
                );
            }
        }
        "liveSessionControl" => match string(step, "operation") {
            "Manage" => window.click("live-sessions-manage", cx),
            operation => {
                let row = session_row(window, step);
                let control = locate::id(&window.within(row).find_by_label(operation));
                locate::click_in(window, "live-session-list", control, cx)
            }
        },
        "expectHealthTooltip" => {
            let key = identity(step);
            let tooltip = window
                .find(format!("health:{key}"))
                .description()
                .map_or_default(str::to_owned);
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
                .map_or_default(str::to_owned);
            assert!(is_exact_timestamp(&tooltip), "event time was {tooltip:?}");
        }
        _ => return false,
    }
    true
}

/// The one header popover row labeled with the step's target. Sessions of one
/// target share that label, so, like the web driver, an exec target keeps only
/// rows with Show (sessions open in a tab), and a step with `local` keeps only
/// the forward whose local URL ends in that port.
fn session_row(window: &mut gpui_kit::Window, step: &Value) -> ElementId {
    let target = string(step, "target");
    let local = step["local"].as_str();
    let candidates: Vec<ElementId> = window
        .within("live-session-list")
        .find_all_by_label(target)
        .into_iter()
        .filter(|row| {
            local.is_none_or(|port| {
                row.description()
                    .and_then(|text| text.split_once(' '))
                    .and_then(|(url, _)| url.rsplit_once(':'))
                    .is_some_and(|(_, row_port)| row_port == port)
            })
        })
        .map(|row| locate::id(&row))
        .collect();
    let rows: Vec<ElementId> = candidates
        .into_iter()
        .filter(|row| {
            target.contains(':')
                || window
                    .within(row.clone())
                    .try_find_by_label("Show")
                    .is_some()
        })
        .collect();
    assert_eq!(rows.len(), 1, "live-session rows for {target} {local:?}");
    rows.into_iter().next().expect("one row")
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
