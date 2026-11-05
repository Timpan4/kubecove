use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    let id = match string(step, "action") {
        "expectOverviewShortcuts" => {
            assert!(seek(window, cx, "page", "overview-shortcuts"));
            let labels = strings(step, "labels");
            assert_eq!(
                window.find("overview-shortcuts").description(),
                Some(labels.len().to_string().as_str())
            );
            for label in labels {
                assert!(
                    window
                        .within("overview-shortcuts")
                        .try_find_by_label(&label)
                        .is_some()
                );
            }
            return true;
        }
        "overviewShortcut" => {
            assert!(seek(window, cx, "page", "overview-shortcuts"));
            let target = locate::id(
                &window
                    .within("overview-shortcuts")
                    .find_by_label(string(step, "label")),
            );
            locate::click_in(window, "page", target, cx);
            return true;
        }
        "expectOverviewScope" => {
            for (index, label) in strings(step, "labels").iter().enumerate() {
                assert_eq!(
                    window.find(format!("overview-scope:{index}")).label(),
                    Some(label.as_str())
                );
            }
            return true;
        }
        "expectCompareTotal" => {
            let id = format!("compare:{}:{}", string(step, "title"), string(step, "side"));
            assert!(seek(window, cx, "page", &id));
            assert!(
                window
                    .find(id)
                    .description()
                    .unwrap()
                    .starts_with(&format!("{} total /", number(step, "total")))
            );
            return true;
        }
        "retryOverview" => "retry-overview".to_owned(),
        "expectOverviewRetry" => {
            assert!(seek(window, cx, "page", "retry-overview"));
            return true;
        }
        "dragKind" => {
            let source = format!("kind-order-item:{}", string(step, "kind"));
            let target = string(step, "target");
            let target = if let Some(lane) = target.strip_prefix("lane:") {
                format!(
                    "kind-order-lane:{}",
                    match lane {
                        "first" => "Show first",
                        "normal" => "Normal",
                        "last" => "Show last",
                        _ => panic!("unknown lane"),
                    }
                )
            } else {
                format!("kind-order-item:{target}")
            };
            reveal(window, "page", &source, cx);
            if string(step, "target").starts_with("lane:") {
                let from = window.find(source).bounds().center();
                let to = window.find(target).bounds();
                window.drag(
                    from,
                    gpui_kit::point(to.center().x, to.bottom() - px(4.)),
                    cx,
                );
            } else {
                window.drag_to(source, target, cx);
            }
            return true;
        }
        "expectEmptyLauncher" => {
            assert!(window.try_find("launcher-context-error").is_none());
            let before = window.find("workspace-create");
            window.click("workspace-create", cx);
            window.render_frame(cx);
            assert_eq!(window.find("workspace-create").label(), before.label());
            assert!(window.try_find("workspace-context:mock-dev").is_none());
            return true;
        }
        "expectEntryCount" => {
            let id = format!("entries:{}", string(step, "panel"));
            assert!(seek(window, cx, "page", &id));
            assert_eq!(
                window.find(id).description(),
                Some(number(step, "count").to_string().as_str())
            );
            return true;
        }
        "copyErrorDetail" => {
            let id = match string(step, "error") {
                "contexts" => "launcher-context-error",
                "namespaces" => "launcher-namespace-error",
                _ => "detail-error",
            };
            window.click(format!("{id}:detail"), cx);
            window.render_frame(cx);
            window.click(format!("{id}:copy"), cx);
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some(string(step, "expected").to_owned())
            );
            window.render_frame(cx);
            assert_eq!(window.find(format!("{id}:copy")).label(), Some("Copied"));
            return true;
        }
        "toggleResourcePin" => {
            let id = format!(
                "pin:{}:{}:{}",
                string(step, "kind"),
                string(step, "namespace"),
                string(step, "name")
            );
            window.render_frame(cx);
            let table = window.find("resource-table").bounds();
            let offset = table.center().x - window.find(id.clone()).bounds().center().x;
            window.scroll(
                "resource-table",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(offset, px(0.))),
                cx,
            );
            locate::click_in(window, "resource-table", id.into(), cx);
            window.scroll(
                "resource-table",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(-offset, px(0.))),
                cx,
            );
            return true;
        }
        "expectWorkspaceEntry" | "openWorkspaceEntry" => {
            let id = format!("entry:{}:{}", string(step, "panel"), number(step, "index"));
            assert!(seek(window, cx, "page", &id));
            assert_eq!(window.find(id.clone()).label(), Some(string(step, "label")));
            assert_eq!(
                window
                    .find(format!("entries:{}", string(step, "panel")))
                    .description(),
                Some(number(step, "count").to_string().as_str())
            );
            if step["action"] == "openWorkspaceEntry" {
                let delta = window.find("page").bounds().center().y
                    - window.find(id.clone()).bounds().center().y;
                window.scroll(
                    "page",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), delta)),
                    cx,
                );
                window.render_frame(cx);
                window.click(id, cx);
            }
            return true;
        }
        "reorderKindKey" => {
            locate::click_in(
                window,
                "page",
                format!("kind-order-item:{}", string(step, "kind")).into(),
                cx,
            );
            window.press(&string(step, "key").replace("Arrow", "").to_lowercase(), cx);
            return true;
        }
        "resetKindOrder" => {
            locate::click_in(window, "page", "reset-kind-order".into(), cx);
            return true;
        }
        "expectRemoteHost" => {
            assert_eq!(
                window.find("remote-host-scope").label(),
                Some(format!("Remote host: {}", string(step, "url")).as_str())
            );
            return true;
        }
        "expectRemoteHostTooltip" => {
            // The web's title attribute and this badge's accessible description carry the same text.
            assert_eq!(
                window.find("remote-host-scope").description(),
                Some(string(step, "value"))
            );
            return true;
        }
        "expectNoRemoteHost" => {
            assert!(window.try_find("remote-host-scope").is_none());
            return true;
        }
        "expectActivePage" => {
            let id = format!("nav:{}", string(step, "page"));
            assert!(seek(window, cx, "sidebar-tree", &id));
            assert_eq!(window.find(id).selected(), Some(true));
            return true;
        }
        "expectGitOpsSelection" => {
            assert!(seek(window, cx, "gitops-list", "gitops-selected"));
            assert_eq!(
                window.find("gitops-selected").label(),
                Some(string(step, "name"))
            );
            return true;
        }
        "configureShellRead" => return true,
        "retryLauncher" => format!("retry-launcher:{}", string(step, "read")).replace(':', "-"),
        "expectLauncherRetry" => {
            assert!(
                window
                    .try_find(format!("retry-launcher-{}", string(step, "read")))
                    .is_some()
            );
            return true;
        }
        "overviewOperation" => format!(
            "open-{}",
            string(step, "destination").to_lowercase().replace(' ', "-")
        ),
        "overviewIncident" => format!("overview-incident:{}", string(step, "filter")),
        "reviewIncidentAction" => {
            locate::click_in(
                window,
                "incident-guide",
                format!("incident-action:{}", number(step, "index")).into(),
                cx,
            );
            return true;
        }
        _ => return false,
    };
    locate::click_in(window, "page", id.into(), cx);
    true
}
