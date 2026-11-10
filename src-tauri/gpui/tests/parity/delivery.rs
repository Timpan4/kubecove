use super::*;

/// Whether the virtualized `list` holds `id` anywhere, not only among its mounted rows.
/// An empty result renders no list, which holds nothing.
fn listed(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, list: &str, id: &str) -> bool {
    let anchor = gpui_kit::ElementId::Name(list.to_owned().into());
    window.try_find(anchor).is_some() && seek(window, cx, list, id)
}

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "openGitOps" => {
            reveal(window, "open-resources", "open-gitops", cx);
            window.click("open-gitops", cx);
        }
        "expandNav" => {
            let page = string(step, "page");
            let toggle = format!("nav-toggle:{page}");
            let id = if window.try_find(toggle.clone()).is_some() {
                toggle
            } else {
                format!("custom-group:{page}")
            };
            reveal(window, "sidebar-tree", &id, cx);
            window.click(id, cx);
        }
        "openGitOpsResources" => {
            let id = format!(
                "gitops-resources:argoApp:mock-dev:{}:{}",
                string(step, "namespace"),
                string(step, "name")
            );
            assert!(seek(window, cx, "gitops-list", &id));
            window.click(id, cx);
        }
        "expectArgoWorkspace" => assert_eq!(
            window.find("argo-workspace-name").label(),
            Some(string(step, "name"))
        ),
        "filterArgoResources" => {
            window.click(format!("argo-filter:{}", string(step, "filter")), cx)
        }
        "setDeliveryView" => window.click(
            format!(
                "{}-view:{}",
                string(step, "page").to_lowercase(),
                string(step, "mode")
            ),
            cx,
        ),
        "expectGitOpsTable" => {
            assert_eq!(
                window.find("gitops-table-title").label(),
                Some(string(step, "title"))
            );
            assert_eq!(
                window.find("gitops-count").label(),
                Some(item_count(number(step, "count")).as_str())
            );
        }
        "expectGitOpsItem" => assert!(seek(window, cx, "gitops-list", &gitops_details(step))),
        "expectNoGitOpsItem" => assert!(!listed(window, cx, "gitops-list", &gitops_details(step))),
        "openGitOpsDetails" => {
            let id = gitops_details(step);
            assert!(seek(window, cx, "gitops-list", &id));
            window.click(id, cx);
        }
        "expectArgoApplication" => assert_eq!(
            window.find("argo-app-status").label(),
            Some(
                format!(
                    "{} {} {}",
                    string(step, "name"),
                    string(step, "health"),
                    string(step, "sync")
                )
                .as_str()
            )
        ),
        "expectArgoSignal" => assert_eq!(
            window
                .find(format!("argo-signal:{}", string(step, "label")))
                .label(),
            Some(number(step, "count").to_string().as_str())
        ),
        "expectHelmCount" => assert_eq!(
            window.find("helm-count").label(),
            Some(item_count(number(step, "count")).as_str())
        ),
        "expectHelmRelease" => assert!(seek(window, cx, "helm-list", &helm_release(step))),
        "expectNoHelmRelease" => assert!(!listed(window, cx, "helm-list", &helm_release(step))),
        "openHelmRelease" => {
            let id = format!(
                "helm-details:{}:{}",
                string(step, "namespace"),
                string(step, "name")
            );
            assert!(seek(window, cx, "helm-list", &id));
            window.click(id, cx);
        }
        "selectHelmNamespace" => {
            window.click(format!("helm-namespace:{}", string(step, "namespace")), cx)
        }
        "searchHelm" => replace_text(window, cx, "helm-search", string(step, "value")),
        "expectHelmDetails" => {
            assert!(seek(window, cx, "helm-list", "helm-details-title"));
            assert_eq!(
                window.find("helm-details-title").label(),
                Some(string(step, "title"))
            );
            assert_eq!(
                window.find("helm-details-description").label(),
                Some(string(step, "description"))
            );
            assert_eq!(
                window.find("helm-details-totals").label(),
                Some(string(step, "totals"))
            );
        }
        _ => return false,
    }
    true
}
