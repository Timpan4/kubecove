use super::*;
pub(super) fn apply(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "replayLiveData" | "mockTiming" | "expectWatchStart" => {}
        "expectLiveState" => {
            if let Some(notice) = step["notice"].as_str() {
                assert_eq!(window.find("live-notice").label(), Some(notice));
            }
            assert_eq!(
                window.find("live-label").label(),
                Some(string(step, "label"))
            );
            if string(step, "label") == "Live" {
                assert!(window.try_find("live-notice").is_none());
            }
        }
        "expectResourceStatus" => assert_eq!(
            window.find(row(step)).description(),
            Some(string(step, "status"))
        ),
        "openCustomKind" => {
            let picker = string(step, "surface") == "picker";
            window.click(
                if picker {
                    "kinds-picker"
                } else {
                    "nav:Custom Resources"
                },
                cx,
            );
            window.render_frame(cx);
            if picker {
                // As the web driver does, search the kind so its entry is in view.
                super::replace_text(window, cx, "kind-search", string(step, "kind"));
                window.render_frame(cx);
            }
            if !picker {
                window.click("custom-group:kustomize.toolkit.fluxcd.io", cx);
                window.render_frame(cx);
            }
            let query = if picker { "pick-kind" } else { "custom-kind" };
            // Table Kind cells carry the same label, so pick the sidebar or picker entry.
            let label = window
                .find_all_by_label(string(step, "kind"))
                .into_iter()
                .find(|label| format!("{:?}", locate::id(label)).contains(query))
                .unwrap_or_else(|| panic!("no {query} entry labelled {}", string(step, "kind")));
            let id = locate::id(&label);
            if !picker || label.description() == Some("unchecked") {
                window.click(id, cx);
            }
            if picker {
                window.click("kinds-picker", cx);
            }
        }
        _ => return false,
    }
    true
}
