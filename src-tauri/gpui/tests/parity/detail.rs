use super::*;

pub(super) fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) -> bool {
    match string(step, "action") {
        "expectYamlAbsent" => {
            let yaml = window.find_by_label("Resource YAML");
            for fragment in strings(step, "fragments") {
                assert!(!yaml.value().unwrap().contains(&fragment));
            }
        }
        "clickActionEntry" => {
            let label = format!("{} controls", string(step, "label"));
            let row = locate::id(&window.find_by_label(&label));
            locate::click_in(window, "actions-scroll", row.clone(), cx);
            let button = locate::id(
                &window
                    .within(row.clone())
                    .find_by_label(string(step, "button")),
            );
            window.within(row).click(button, cx);
        }
        "fillContainerImage" => {
            let id = format!("image:{}", string(step, "container"));
            reveal(window, "actions-scroll", &id, cx);
            window.click(id, cx);
            window.press("secondary-a", cx);
            window.input(string(step, "value"), cx);
        }
        "openActionEditor" => {
            let id = format!("action-{}", string(step, "row"));
            reveal(window, "actions-scroll", &id, cx);
            window.click(id, cx);
        }
        "fillActionField" => {
            let label = string(step, "label");
            reveal(window, "actions-scroll", label, cx);
            window.click(gpui_kit::ElementId::Name(label.to_owned().into()), cx);
            window.press("secondary-a", cx);
            window.input(string(step, "value"), cx);
        }
        "chooseActionRevision" => {
            let id = format!("action-revision:{}", number(step, "revision"));
            reveal(window, "actions-scroll", &id, cx);
            window.click(id, cx);
        }
        "selectYamlShape" | "selectYamlEncoding" => {
            let picker = if string(step, "action") == "selectYamlShape" {
                "shape"
            } else {
                "encoding"
            };
            window.click(format!("yaml-{picker}"), cx);
        }
        "expectYamlPicker" => assert_eq!(
            window.find_by_label(string(step, "label")).value(),
            Some(string(step, "value"))
        ),
        "replaceYamlDraft" => {
            reveal(window, "detail-body", "yaml-editor", cx);
            let target = locate::id(&window.within("detail-body").find_by_label("Resource YAML"));
            window.within("detail-body").click(target, cx);
            window.press("secondary-a", cx);
            window.input(string(step, "value"), cx);
            window.render_frame(cx);
            let edited = window.within("detail-body").find_by_label("Resource YAML");
            assert_eq!(
                edited.value(),
                Some(string(step, "value")),
                "YAML replacement did not reach the editor: editor {:?}, body {:?}",
                edited.bounds(),
                window.find("detail-body").bounds()
            );
        }
        "formatYaml" => {
            reveal(window, "detail-body", "yaml-format", cx);
            window.click("yaml-format", cx);
        }
        "expectYamlDocument" => {
            let yaml = window.find_by_label("Resource YAML");
            let actual: serde_json::Value = serde_yaml::from_str(yaml.value().unwrap()).unwrap();
            let expected: serde_json::Value = serde_json::from_str(string(step, "value")).unwrap();
            assert_eq!(actual, expected);
        }
        "openResources" => {
            if window.try_find("resources").is_none() {
                window.click("open-resources", cx);
            }
            window.render_frame(cx);
            assert!(window.find("resources").visible());
        }
        "search" => {
            window.click("search", cx);
            window.press("secondary-a", cx);
            window.input(string(step, "value"), cx);
        }
        // Expectations on rows mean rendered in the table, possibly scrolled out of
        // view, as Playwright's `visible` state does.
        "expectResource" => assert!(reveal_resource(window, &row(step), cx)),
        "openResource" => {
            let row = row(step);
            assert!(reveal_resource(window, &row, cx));
            window.click(row, cx);
        }
        "expectDetail" => assert_eq!(
            window.find("detail-identity").label(),
            Some(
                format!(
                    "{} {} {}",
                    string(step, "kind"),
                    string(step, "namespace"),
                    string(step, "name")
                )
                .as_str()
            )
        ),
        "expectDetailTab" => {
            let id = format!("show-{}", string(step, "tab").to_lowercase());
            assert_eq!(window.find(id).selected(), Some(true));
        }
        "expectClusterResource" => {
            assert!(reveal_resource(window, &cluster_row(step), cx))
        }
        "expectNoClusterResource" => {
            assert!(!reveal_resource(window, &cluster_row(step), cx))
        }
        "showYaml" => window.click("show-yaml", cx),
        "expectYaml" => {
            let yaml = window.find_by_label("Resource YAML");
            let value = yaml.value().unwrap();
            for expected in step["contains"].as_array().unwrap() {
                assert!(
                    value.contains(expected.as_str().unwrap()),
                    "missing {expected} in {value}"
                );
            }
        }
        "closeDetail" => window.click("close-detail", cx),
        "expectDetailClosed" => assert!(window.try_find("detail-identity").is_none()),
        "expectEmpty" => assert!(window.find("empty").visible()),
        "refresh" => {
            window.click("live-status", cx);
            window.render_frame(cx);
            window.click("refresh", cx);
        }
        "expectSearch" => {
            assert_eq!(window.find("search").value(), Some(string(step, "value")))
        }
        "openPage" => window.click(format!("nav:{}", string(step, "page")), cx),
        "showTab" => window.click(format!("show-{}", string(step, "tab").to_lowercase()), cx),
        "filterEvents" => {
            let id = match string(step, "filter") {
                "All" => "events-filter-all",
                "Warnings" => "events-filter-warning",
                "Normal" => "events-filter-normal",
                other => panic!("unknown event filter: {other}"),
            };
            window.click(id, cx);
        }
        "expectDetailScroll" => super::detail_scroll::verify(window, cx, step),
        "expectEventCount" => {
            let count = number(step, "count");
            if let Some(last) = count.checked_sub(1) {
                assert!(window.try_find(format!("event:{last}")).is_some());
            }
            assert!(window.try_find(format!("event:{count}")).is_none());
        }
        "expectEvent" => assert_eq!(
            window
                .find(format!("event:{}", number(step, "index")))
                .label(),
            Some(format!("{} {}", string(step, "reason"), string(step, "message")).as_str())
        ),
        "expectLiveSessionCount" => assert_eq!(
            window.find("live-sessions").label(),
            Some(format!("Open live sessions ({} active)", number(step, "count")).as_str())
        ),
        "filterIncidents" => {
            window.click(format!("incident-filter:{}", string(step, "filter")), cx)
        }
        "expectIncidentGuide" => assert_eq!(
            window.find("incident-guide-title").label(),
            Some(string(step, "title"))
        ),
        "expectGitOpsCount" => assert_eq!(
            window
                .find(format!("gitops-count:{}", string(step, "label")))
                .label(),
            Some(number(step, "count").to_string().as_str())
        ),
        "expectPortForward" => assert!(window.find_by_label(string(step, "title")).visible()),
        "expectLogLines" => {
            // Rows are labeled with their headline, in display order.
            let rendered: Vec<String> = (0usize..)
                .map_while(|index| window.try_find(("log-row", index)))
                .map(|row| row.label().unwrap_or_default().to_owned())
                .collect();
            assert_eq!(rendered, strings(step, "lines"));
        }
        "filterLogs" => replace_text(window, cx, "log-filter", string(step, "value")),
        "toggleLogLevel" => window.click(format!("log-level-{}", string(step, "level")), cx),
        "expectLogsEmpty" => assert_eq!(
            window.find("logs-empty").label(),
            Some(string(step, "title"))
        ),
        "selectRevision" => {
            let id = format!("revision:{}", string(step, "label"));
            reveal(window, DETAIL_TOP, &id, cx);
            window.click(id, cx);
        }
        "expectRevisionDiff" => {
            assert_eq!(
                window.find("revision-header").label(),
                Some(
                    format!(
                        "{} compared with {}",
                        string(step, "selected"),
                        string(step, "comparison")
                    )
                    .as_str()
                )
            );
            contains_lines(
                window.find("revision-diff").label(),
                &strings(step, "contains"),
            );
        }
        "reviewAction" => {
            let id = match string(step, "label") {
                "Restart" => "action-restart",
                "Delete" => "action-delete",
                "Pause" | "Resume" => "action-rollout",
                "Check changes" => "action-check",
                "Review rollback" => "action-rollback-review",
                other => panic!("unknown resource action: {other}"),
            };
            reveal(window, "actions-scroll", id, cx);
            window.click(id, cx);
        }
        "editReplicas" => {
            reveal(window, "actions-scroll", "action-replicas", cx);
            window.click("action-replicas", cx);
            window.render_frame(cx);
            replace_text(window, cx, "action-replicas-input", string(step, "value"));
        }
        "expectReview" => {
            assert_eq!(
                window.find("review-title").label(),
                Some(string(step, "title"))
            );
            assert_eq!(
                window.find("review-description").label(),
                Some(string(step, "description"))
            );
            contains_lines(
                window.find("review-diff").label(),
                &strings(step, "contains"),
            );
        }
        "confirmDeletion" => replace_text(window, cx, "review-confirmation", string(step, "value")),
        "executeReview" => window.click("review-execute", cx),
        "closeReview" => window.click("review-cancel", cx),
        "expectNoReview" => assert!(window.try_find("review-title").is_none()),
        "expectActionResult" => assert_eq!(
            window.find("action-result").label(),
            Some(string(step, "text"))
        ),
        "expectNoActionResult" => assert!(window.try_find("action-result").is_none()),
        "editYaml" => {
            reveal(window, "detail-body", "edit-draft", cx);
            window.click("edit-draft", cx);
        }
        "applyYaml" => window.click("yaml-apply", cx),
        "dryRunYaml" => window.click("yaml-dry-run", cx),
        "expectDryRun" => contains_lines(
            window.find("yaml-dry-run-diff").label(),
            &strings(step, "contains"),
        ),
        "expectYamlNotice" => assert!(window.find_by_label(string(step, "title")).visible()),
        "expectNoYamlNotice" => assert!(window.try_find_by_label(string(step, "title")).is_none()),
        "expectYamlReadOnly" => {
            assert!(window.try_find("edit-draft").is_some());
            assert!(window.try_find("yaml-dry-run").is_none());
        }
        "expectNoResource" => assert!(!reveal_resource(window, &row(step), cx)),
        _ => return false,
    }
    true
}
