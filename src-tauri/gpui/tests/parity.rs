//! Runs the shared scenarios in `e2e/parity/scenarios/` against the GPUI client
//! with the browser-mock fixture the web driver reads. Driven by
//! `bun scripts/e2e-parity.ts`, which exports the fixture and checks both reports.

mod common;
#[path = "parity/locate.rs"]
mod locate;
#[path = "parity/scroll.rs"]
mod scroll;
#[path = "parity/topology_metrics.rs"]
mod topology_metrics;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use common::Fixture;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::Inspector;
use scroll::{reveal, seek};
use serde_json::{Value, json};

fn string<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_else(|| panic!("missing {key}"))
}
fn number(v: &Value, key: &str) -> u64 {
    v[key].as_u64().unwrap_or_else(|| panic!("missing {key}"))
}
fn row(step: &Value) -> String {
    format!(
        "resource:{}:{}:{}",
        string(step, "kind"),
        string(step, "namespace"),
        string(step, "name")
    )
}

/// The first detail tab, at the top of the scrolling detail body.
const DETAIL_TOP: &str = "show-details";
/// The RBAC category bar, at the top of the page's scroll area.
const RBAC_TOP: &str = "rbac-category:Roles";

/// The Details button of a GitOps card or list row.
fn gitops_details(step: &Value) -> String {
    format!(
        "gitops-details:{}:{}:{}",
        string(step, "kind"),
        string(step, "namespace"),
        string(step, "name")
    )
}

fn helm_release(step: &Value) -> String {
    format!(
        "helm-release:{}:{}",
        string(step, "namespace"),
        string(step, "name")
    )
}

fn item_count(count: u64) -> String {
    format!("{count} item{}", if count == 1 { "" } else { "s" })
}

fn strings(step: &Value, key: &str) -> Vec<String> {
    step[key]
        .as_array()
        .unwrap_or_else(|| panic!("missing {key}"))
        .iter()
        .map(|item| item.as_str().unwrap().to_owned())
        .collect()
}

/// Every expected line appears as a whole line of the element's label.
fn contains_lines(label: Option<&str>, expected: &[String]) {
    let label = label.expect("diff has a label");
    for line in expected {
        assert!(
            label.lines().any(|rendered| rendered == line),
            "missing {line:?} in {label:?}"
        );
    }
}

/// Focuses an input and replaces its text, as Playwright's `fill` does.
fn replace_text(
    window: &mut gpui_kit::Window,
    cx: &mut gpui_kit::App,
    id: &'static str,
    text: &str,
) {
    window.click(id, cx);
    window.press("secondary-a", cx);
    window.input(text, cx);
}

/// A palette result: its name, described by kind and namespace.
fn palette_resource(
    window: &mut gpui_kit::Window,
    step: &Value,
) -> Option<gpui_kit::test::ElementSnapshot> {
    let description = format!("{} {}", string(step, "kind"), string(step, "namespace"));
    locate::described(window, "palette-list", string(step, "name"), &description)
}

fn cluster_row(step: &Value) -> String {
    format!(
        "resource:{}::{}",
        string(step, "kind"),
        string(step, "name")
    )
}

/// Every scenario file in name order, as `loadContract` in `e2e/parity/contract.ts` reads them.
fn scenarios() -> Vec<Value> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../e2e/parity/scenarios");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{} unreadable: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .flat_map(|file| {
            let contract: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
            assert_eq!(contract["fixture"], "browser-mocks", "{}", file.display());
            assert_eq!(contract["context"], "mock-dev", "{}", file.display());
            contract["scenarios"].as_array().unwrap().clone()
        })
        .collect()
}

fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) {
    if topology_metrics::apply(window, cx, step) {
        return;
    }
    match string(step, "action") {
        "openResources" => {
            window.click("open-resources", cx);
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
        "expectResource" => assert!(window.try_find(row(step)).is_some()),
        "openResource" => {
            let row = row(step);
            reveal(window, "resources", &row, cx);
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
        "expectClusterResource" => {
            assert!(window.try_find(cluster_row(step)).is_some())
        }
        "expectNoClusterResource" => {
            assert!(window.try_find(cluster_row(step)).is_none())
        }
        "showYaml" => window.click("show-yaml", cx),
        "expectYaml" => {
            let yaml = window.find_by_label("Resource YAML");
            let value = yaml.value().unwrap();
            for expected in step["contains"].as_array().unwrap() {
                assert!(value.contains(expected.as_str().unwrap()));
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
        "editYaml" => window.click("edit-draft", cx),
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
        "expectNoResource" => assert!(window.try_find(row(step)).is_none()),
        "openLauncher" => window.click("header-workspaces", cx),
        "expectSavedWorkspace" => assert_eq!(
            window
                .within("launcher")
                .find_by_label(string(step, "name"))
                .description(),
            Some(string(step, "summary"))
        ),
        "openSavedWorkspace" => {
            let label = format!("Open {}", string(step, "name"));
            locate::click_label(window, "launcher-body", &label, cx);
        }
        "nameWorkspace" => replace_text(window, cx, "workspace-name", string(step, "value")),
        "selectWorkspaceContext" => {
            window.click("workspace-context", cx);
            window.render_frame(cx);
            window.click(format!("workspace-context:{}", string(step, "context")), cx);
        }
        "toggleWorkspaceNamespace" => {
            let id = format!("workspace-namespace:{}", string(step, "namespace"));
            locate::click_in(window, "workspace-namespaces", id.into(), cx);
        }
        "createWorkspace" => window.click("workspace-create", cx),
        "expectOpenWorkspace" => {
            let title = window.find("workspace-title");
            assert_eq!(title.label(), Some(string(step, "name")));
            let context = window.find("scope-context");
            assert_eq!(context.label(), Some(string(step, "context")));
        }
        "selectRbacCategory" => locate::click_in(
            window,
            "page",
            format!("rbac-category:{}", string(step, "category")).into(),
            cx,
        ),
        "expectRbacCategory" => assert_eq!(
            window
                .find(format!("rbac-category:{}", string(step, "category")))
                .selected(),
            Some(true)
        ),
        "filterRbacRisk" => {
            window.click(format!("rbac-risk-filter:{}", string(step, "filter")), cx)
        }
        "expectRbacItem" => assert_eq!(
            window
                .within("rbac-queue")
                .find_by_label(string(step, "title"))
                .description(),
            Some(string(step, "risk"))
        ),
        "expectNoRbacItem" => assert!(
            window
                .within("rbac-queue")
                .try_find_by_label(string(step, "title"))
                .is_none()
        ),
        "selectRbacItem" => {
            let target = locate::id(
                &window
                    .within("rbac-queue")
                    .find_by_label(string(step, "title")),
            );
            locate::click_in(window, "page", target, cx);
        }
        "editRbacReviewNote" => {
            locate::click_in(window, "page", "rbac-review-editor".into(), cx);
            window.press("secondary-a", cx);
            window.input(string(step, "value"), cx);
        }
        "expectRbacReviewNote" => assert_eq!(
            window.find_by_label("RBAC review note").value(),
            Some(string(step, "value"))
        ),
        "saveRbacReview" => locate::click_in(
            window,
            "page",
            format!("rbac-mark-{}", string(step, "disposition")).into(),
            cx,
        ),
        "expectRbacReview" => assert_eq!(
            window.find("rbac-review-status").label(),
            Some(string(step, "label"))
        ),
        "clearRbacReview" => locate::click_in(window, "page", "rbac-clear-review".into(), cx),
        "investigateRbacSubject" => {
            window.click(format!("rbac-subject:{}", string(step, "subject")), cx)
        }
        "expectRbacIdentity" => assert_eq!(
            window.find("rbac-identity").label(),
            Some(string(step, "identity"))
        ),
        "expectRbacGrant" => assert!(
            !window
                .within("rbac-grants")
                .find_all_by_label(string(step, "grant"))
                .is_empty()
        ),
        "expectNoRbacGrant" => assert!(
            window
                .within("rbac-grants")
                .find_all_by_label(string(step, "grant"))
                .is_empty()
        ),
        "verifyRbacAccess" => {
            // The verifier sits below the fold of the page's scroll area. The
            // click follows in `finish`, after the edit's change events apply.
            reveal(window, RBAC_TOP, "rbac-verb", cx);
            replace_text(window, cx, "rbac-verb", string(step, "verb"));
        }
        "expectRbacVerdict" => assert_eq!(
            window.find("rbac-verdict").label(),
            Some(string(step, "outcome"))
        ),
        "openSettings" => window.click("open-settings", cx),
        "closeSettings" => window.click("settings-back", cx),
        "showSettingsSection" => {
            window.click(format!("settings-section:{}", string(step, "section")), cx)
        }
        "searchSettings" => replace_text(window, cx, "settings-search", string(step, "value")),
        "expectSettingsHeading" => assert_eq!(
            window.find("settings-heading").label(),
            Some(string(step, "heading"))
        ),
        "expectSettingRow" => assert!(
            window
                .try_find(format!("setting-row:{}", string(step, "title")))
                .is_some()
        ),
        "expectNoSettingRow" => assert!(
            window
                .try_find(format!("setting-row:{}", string(step, "title")))
                .is_none()
        ),
        "expectSettingsEmpty" => assert_eq!(
            window.find("settings-empty").label(),
            Some(format!("No settings match \"{}\".", string(step, "value")).as_str())
        ),
        "setKubeconfigEnvVar" => {
            replace_text(window, cx, "kubeconfig-env-var", string(step, "value"));
            window.click("kubeconfig-env-save", cx);
        }
        "expectKubeconfigEnvVar" => assert_eq!(
            window.find("kubeconfig-env-var").value(),
            Some(string(step, "value"))
        ),
        "toggleSetting" => window.click(format!("setting:{}", string(step, "setting")), cx),
        "expectSetting" => assert_eq!(
            window
                .find(format!("setting:{}", string(step, "setting")))
                .checked(),
            Some(step["checked"].as_bool().expect("missing checked"))
        ),
        "openPalette" => {
            match string(step, "via") {
                // Cmd on macOS, Ctrl elsewhere, like the web's `ControlOrMeta+K`.
                "shortcut" => window.press("secondary-k", cx),
                "button" => window.click("open-palette", cx),
                other => panic!("unknown palette opener: {other}"),
            }
            window.render_frame(cx);
            assert!(window.find("palette-dialog").visible());
        }
        "searchPalette" => replace_text(window, cx, "palette-query", string(step, "value")),
        "expectPaletteResource" => assert!(palette_resource(window, step).is_some()),
        "expectNoPaletteResource" => assert!(palette_resource(window, step).is_none()),
        "choosePaletteResource" => {
            let target = locate::id(&palette_resource(window, step).expect("palette resource"));
            locate::click_in(window, "palette-list", target, cx);
        }
        "expectPaletteEntry" => assert!(
            window
                .within("palette-list")
                .try_find_by_label(string(step, "label"))
                .is_some()
        ),
        "expectNoPaletteEntry" => assert!(
            window
                .within("palette-list")
                .try_find_by_label(string(step, "label"))
                .is_none()
        ),
        "choosePaletteEntry" => {
            locate::click_label(window, "palette-list", string(step, "label"), cx)
        }
        "openGitOps" => {
            reveal(window, "open-resources", "open-gitops", cx);
            window.click("open-gitops", cx);
        }
        "expandNav" => window.click(format!("nav-toggle:{}", string(step, "page")), cx),
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
        "expectNoGitOpsItem" => assert!(window.try_find(gitops_details(step)).is_none()),
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
        "expectNoHelmRelease" => assert!(window.try_find(helm_release(step)).is_none()),
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
        action => panic!("unknown parity action: {action}"),
    }
}

/// The second half of steps that type and then click. Input change events
/// apply when the update that typed ends, as they do between a user's
/// keystrokes and a later click.
fn finish(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) {
    if string(step, "action") == "verifyRbacAccess" {
        reveal(window, RBAC_TOP, "rbac-verify", cx);
        window.click("rbac-verify", cx);
    }
}

#[gpui_kit::test]
fn shared_inspection_scenarios(cx: &mut TestAppContext) {
    let artifacts =
        std::env::var_os("KUBECOVE_E2E_ARTIFACTS").expect("KUBECOVE_E2E_ARTIFACTS is required");
    let fixture = std::env::var_os("KUBECOVE_PARITY_FIXTURE")
        .expect("KUBECOVE_PARITY_FIXTURE is required; run `bun scripts/e2e-parity.ts`");
    let report = Path::new(&artifacts).join("gpui.json");
    match fs::remove_file(&report) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("cannot remove stale parity report: {error}"),
    }
    let scenarios = scenarios();
    assert!(!scenarios.is_empty());
    let mut passed = Vec::new();
    for scenario in &scenarios {
        let id = string(scenario, "id");
        cx.update(gpui_kit::init);
        // A fresh fixture per scenario, as the web driver reloads the page and its
        // in-memory mock state.
        let backend = Arc::new(Fixture::load(Path::new(&fixture)));
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    // The web driver's viewport.
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1440.), px(900.)),
                    })),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        Inspector::new(backend, window, cx)
                            .with_workspace_name("Parity mock workspace")
                    })
                },
            )
            .unwrap()
        });
        cx.run_until_parked();
        for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
            println!("parity {id} step {index}: {step}");
            // Reads a render starts must settle before the step observes or acts.
            cx.update_window(handle, |_, window, cx| window.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                act(window, cx, step);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                finish(window, cx, step);
            })
            .unwrap();
            cx.run_until_parked();
        }
        passed.push(json!({"id": id, "status": "passed"}));
    }
    fs::create_dir_all(&artifacts).unwrap();
    fs::write(
        report,
        serde_json::to_vec_pretty(&json!({"driver": "gpui", "scenarios": passed})).unwrap(),
    )
    .unwrap();
}
