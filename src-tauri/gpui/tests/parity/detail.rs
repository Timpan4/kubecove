use super::*;
use gpui_kit::{Bounds, ElementId, Pixels, Point, point};

fn tab_id(label: &str) -> String {
    if label == "Argo CD" {
        "show-argo".into()
    } else {
        format!("show-{}", label.to_lowercase())
    }
}

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
            locate::click_in(window, "sidebar-tree", "nav:Workspace Overview".into(), cx);
            window.render_frame(cx);
            locate::click_in(window, "page", "open-resources".into(), cx);
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
            let row = resolve_resource(window, &row(step), cx).expect("resource row is rendered");
            window.within("resource-table").click(row, cx);
        }
        "expectAmbiguousResource" => assert!(
            lookup_resource(window, &row(step), cx).is_err(),
            "expected an ambiguous resource row: {step}"
        ),
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
            let id = tab_id(string(step, "tab"));
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
            // The menu stays open after a refresh, so close it before later steps click under it.
            window.render_frame(cx);
            window.click("live-status", cx);
        }
        "expectSearch" => {
            assert_eq!(window.find("search").value(), Some(string(step, "value")))
        }
        "openPage" => window.click(format!("nav:{}", string(step, "page")), cx),
        "showTab" => {
            let id = tab_id(string(step, "tab"));
            reveal(window, "detail-body", &id, cx);
            window.click(id, cx);
        }
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
            let rendered: Vec<String> = locate::mounted_rows(window, "log-row")
                .into_iter()
                .map(|(_, label)| label)
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
        // A page without a table, such as a held loading state, has no rows, as on the web.
        "expectNoResource" => assert!(
            window.try_find("resource-table").is_none() || !reveal_resource(window, &row(step), cx)
        ),
        _ => return false,
    }
    true
}

/// Virtualized rows do not have test elements until the table scrolls to them.
pub(super) fn reveal_resource(
    window: &mut gpui_kit::Window,
    target: &str,
    cx: &mut gpui_kit::App,
) -> bool {
    resolve_resource(window, target, cx).is_some()
}

/// Scrolls the table to the row that `target` names and returns its element ID.
/// Row IDs end in the row's context, so a target without one matches the row
/// of every context. Like the web driver, a target that names more than one
/// row is an error rather than a choice that depends on the scroll position.
pub(super) fn resolve_resource(
    window: &mut gpui_kit::Window,
    target: &str,
    cx: &mut gpui_kit::App,
) -> Option<String> {
    lookup_resource(window, target, cx)
        .unwrap_or_else(|rows| panic!("Resource row is ambiguous: {rows:?}"))
}

fn lookup_resource(
    window: &mut gpui_kit::Window,
    target: &str,
    cx: &mut gpui_kit::App,
) -> Result<Option<String>, Vec<String>> {
    // A target that names its context, `resource:{kind}:{namespace}:{name}:{context}`,
    // is one row's whole ID, since kinds, namespaces and names hold no `:`.
    let names_context = target.splitn(5, ':').count() == 5;
    // Only context-free targets in a multi-context workspace can match
    // several clusters. Check the whole page before using any of those rows.
    if names_context || window.try_find("context-picker").is_none() {
        return Ok(find_resource(window, target, names_context, cx));
    }
    let rows = table_rows(window, target, cx);
    match rows.as_slice() {
        [] => Ok(None),
        [id] => Ok(Some(
            find_resource(window, id, true, cx)
                .unwrap_or_else(|| panic!("row {id} is no longer rendered")),
        )),
        _ => Err(rows),
    }
}

fn find_resource(
    window: &mut gpui_kit::Window,
    target: &str,
    exact: bool,
    cx: &mut gpui_kit::App,
) -> Option<String> {
    let mut found = None;
    scan_table(window, cx, |window| {
        let viewport = table_viewport(window);
        found = rendered_rows(window, target).into_iter().find(|id| {
            (!exact || id == target)
                && viewport.contains(&window.find(id.clone()).bounds().center())
        });
        found.is_some()
    });
    found
}

/// The element IDs of every row on the current table page that `target`
/// names, top to bottom. The table mounts only the rows near its viewport, so
/// the whole page is scanned from the top.
pub(super) fn table_rows(
    window: &mut gpui_kit::Window,
    target: &str,
    cx: &mut gpui_kit::App,
) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    prepare_table(window, cx);
    scan_table_from_top(window, cx, |window| {
        for id in rendered_rows(window, target) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        false
    });
    ids
}

/// The element IDs of the visible rows that `target` names, top to bottom.
fn rendered_rows(window: &gpui_kit::Window, target: &str) -> Vec<String> {
    let prefix = format!("{target}:");
    let table = ElementId::Name("resource-table".into());
    window
        .elements()
        .iter()
        .filter(|element| element.visible() && element.path().contains(&table))
        .filter_map(|element| {
            let id = locate::id(element).to_string();
            (id == target || id.starts_with(&prefix)).then_some(id)
        })
        .collect()
}

/// Checks the current viewport first, then scans the table from the top.
pub(super) fn scan_table(
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
    mut visit: impl FnMut(&gpui_kit::Window) -> bool,
) -> bool {
    prepare_table(window, cx);
    visit(window) || scan_table_from_top(window, cx, visit)
}

fn prepare_table(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    window.render_frame(cx);
    if !window.find("resource-table").visible() {
        reveal(window, "page", "resource-table", cx);
    }
    // A cell lookup can leave the Name buttons clipped horizontally.
    let table = window.find("resource-table").bounds();
    let name = window.find("sort:Name").bounds();
    if name.left() < table.left() {
        scroll_table(window, cx, point(table.left() - name.left(), px(0.)));
    }
}

/// Walks the current pagination page from top to bottom. Compare the Name
/// header's position within the table to detect when the inner scroller stops.
fn scan_table_from_top(
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
    mut visit: impl FnMut(&gpui_kit::Window) -> bool,
) -> bool {
    let offset = |window: &gpui_kit::Window| {
        window.find("sort:Name").bounds().top() - window.find("resource-table").bounds().top()
    };
    reveal(window, "page", "resource-table", cx);
    if offset(window) < px(0.) {
        // The Name header starts inside the table's top padding.
        let table = window.find("resource-table").bounds();
        scroll_table(
            window,
            cx,
            point(px(0.), -offset(window) + table.size.height),
        );
    }
    loop {
        if visit(window) {
            return true;
        }
        let before = offset(window);
        scroll_table(
            window,
            cx,
            point(px(0.), -table_viewport(window).size.height / 2.),
        );
        if offset(window) >= before {
            return false;
        }
    }
}

fn table_viewport(window: &gpui_kit::Window) -> Bounds<Pixels> {
    window
        .find("resource-table")
        .bounds()
        .intersect(&window.find("page").bounds())
}

fn scroll_table(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, delta: Point<Pixels>) {
    use gpui_kit::test::TestEventExt;
    use gpui_kit::{InputEvent, ScrollDelta, ScrollEvent};

    let position = table_viewport(window).center();
    window.pointer_move(position, None, cx);
    window.dispatch_event(
        ScrollEvent {
            position,
            delta: ScrollDelta::Pixels(delta),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    // The same wheel event can move the page when the detail stacks the
    // table below the map. Bring the table's viewport back before reading rows.
    reveal(window, "page", "resource-table", cx);
}
