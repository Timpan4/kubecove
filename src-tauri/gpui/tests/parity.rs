//! Runs the shared scenarios in `e2e/parity/scenarios/` against the GPUI client
//! with the browser-mock fixture the web driver reads. Driven by
//! `bun scripts/e2e-parity.ts`, which exports the fixture and checks both reports.

#[path = "parity/adapters.rs"]
mod adapters;
#[path = "parity/argo.rs"]
mod argo;
mod common;
#[path = "parity/delivery.rs"]
mod delivery;
#[path = "parity/detail.rs"]
mod detail;
#[path = "parity/live_data.rs"]
mod live_data;
#[path = "parity/live_sessions.rs"]
mod live_sessions;
#[path = "parity/locate.rs"]
mod locate;
#[path = "parity/mcp.rs"]
mod mcp;
#[path = "parity/scroll.rs"]
mod scroll;
#[path = "parity/settings_integrations.rs"]
mod settings_integrations;
#[path = "parity/shell.rs"]
mod shell;
#[path = "parity/sidebar_live.rs"]
mod sidebar_live;
#[path = "parity/terminal.rs"]
mod terminal;
#[path = "parity/topology_metrics.rs"]
mod topology_metrics;
#[path = "parity/workspace.rs"]
mod workspace;
#[path = "parity/workspace_editing.rs"]
mod workspace_editing;

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
    if text.is_empty() {
        window.press("backspace", cx);
    } else {
        window.input(text, cx);
    }
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
    let prefix = std::env::var("KUBECOVE_PARITY_SCENARIO_PREFIX").unwrap_or_default();
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
        .filter(|scenario| string(scenario, "id").starts_with(&prefix))
        .collect()
}

fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) {
    if !(shell::act(window, cx, step)
        || argo::act(window, cx, step)
        || adapters::act(window, cx, step)
        || mcp::act(window, cx, step)
        || live_sessions::apply(window, cx, step)
        || terminal::apply(window, cx, step)
        || settings_integrations::act(window, cx, step)
        || live_data::apply(window, cx, step)
        || sidebar_live::apply(window, cx, step)
        || topology_metrics::apply(window, cx, step)
        || detail::act(window, cx, step)
        || workspace::act(window, cx, step)
        || workspace_editing::act(window, cx, step)
        || delivery::act(window, cx, step))
    {
        panic!("unknown parity action: {}", string(step, "action"));
    }
}

/// The second half of steps that type and then click. Input change events
/// apply when the update that typed ends, as they do between a user's
/// keystrokes and a later click.
fn finish(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) {
    if step["action"] == "selectArgoPreference" {
        window.press("enter", cx);
    }
    if step["action"] == "connectArgoProfile" {
        let anchor = if step["surface"] == "page" {
            "argo-connection-dialog"
        } else {
            "detail-body"
        };
        if step["tunnel"] == true {
            let id = "argo-server:service:argocd:argocd-server:443";
            reveal(window, anchor, id, cx);
            window.click(id, cx);
            window.render_frame(cx);
        }
        if step["local"] == true {
            reveal(window, anchor, "argo-username", cx);
            replace_text(window, cx, "argo-username", "fixture-user");
            reveal(window, anchor, "argo-password", cx);
            replace_text(window, cx, "argo-password", "fixture-password");
        } else {
            reveal(window, anchor, "argo-token", cx);
            replace_text(window, cx, "argo-token", "parity-fixture-token");
        }
        if let Some(ca) = step["customCa"].as_str() {
            reveal(window, anchor, "argo-ca", cx);
            replace_text(window, cx, "argo-ca", ca);
        }
        reveal(window, anchor, "argo-connect-submit", cx);
        window.click("argo-connect-submit", cx);
    }
    if step["action"] == "configureArgoSync" {
        reveal(window, "detail-body", "argo-sync-advanced", cx);
        window.click("argo-sync-advanced", cx);
    }
    if step["action"] == "reopenStoredSettings" {
        window.click("open-settings", cx);
    }
    if matches!(
        string(step, "action"),
        "selectYamlShape" | "selectYamlEncoding"
    ) {
        window.press(
            if matches!(string(step, "value"), "Apply clean" | "KYAML") {
                "down"
            } else {
                "up"
            },
            cx,
        );
        window.press("enter", cx);
    }
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
    fs::create_dir_all(&artifacts).unwrap();
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
        let (mut handle, _) = cx.update(|cx| {
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
                        Inspector::new(backend.clone(), window, cx)
                            .with_workspace_name("Parity mock workspace")
                    })
                },
            )
            .unwrap()
        });
        cx.run_until_parked();
        for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
            if step["action"] == "configureArgoInventory" {
                *backend.argo.inventory_mode.lock().unwrap() = string(step, "mode").into();
            }
            if step["action"] == "configureArgoMock" {
                *backend.argo.script.lock().unwrap() = string(step, "script").into();
                if step["script"] == "snapshot-error" {
                    cx.background_executor
                        .advance_clock(std::time::Duration::from_millis(15_000));
                    cx.run_until_parked();
                }
            }
            if step["action"] == "configureShellRead" {
                *backend.shell_read_phase.lock().unwrap() = string(step, "phase").to_owned();
            }
            if step["action"] == "failWorkspaceContext" {
                *backend.failed_workspace_context.lock().unwrap() =
                    Some(string(step, "context").to_owned());
            }
            if step["action"] == "expectMcpRequests" {
                assert_eq!(backend.mcp_requests(), strings(step, "requests"));
            }
            if step["action"] == "expectMcpCopy" {
                use kubecove_gpui::backend::Backend;
                let preferences =
                    serde_json::to_string(&backend.load_preferences().unwrap()).unwrap();
                assert!(
                    !preferences.contains("parity-mcp"),
                    "MCP credentials must not enter preferences"
                );
            }
            if step["action"] == "expectDocumentationRequest" {
                assert_eq!(backend.documentation_requests(), vec![string(step, "url")]);
            }
            if step["action"] == "restartClient" {
                cx.update_window(handle, |_, window, _| window.remove_window())
                    .unwrap();
                let (reopened, _) = cx.update(|cx| {
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
                                Inspector::new(backend.clone(), window, cx)
                                    .with_workspace_name("Parity mock workspace")
                            })
                        },
                    )
                    .unwrap()
                });

                handle = reopened;
                cx.run_until_parked();
            }
            if step["action"] == "reopenStoredSettings" {
                backend.reopen_settings_storage();
            }
            if step["action"] == "configureSettingsMock" {
                backend.settings_script(string(step, "script"));
            }
            if step["action"] == "expectRuntimeEndpoint" {
                assert_eq!(backend.runtime_endpoint(), string(step, "value"));
            }
            if step["action"] == "expectWorkspaceExport" {
                backend.expect_workspace_export(string(step, "name"));
            }
            if step["action"] == "replayLiveData" {
                backend.replay_live(string(step, "script"));
            }
            if step["action"] == "replayPageReads" {
                backend.replay_page_reads(string(step, "phase"));
                cx.run_until_parked();
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(250));
                cx.run_until_parked();
            }
            if step["action"] == "focusStalePage" {
                gpui_kit::VisualTestContext::from_window(handle, cx).deactivate_window();
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(
                        if step["page"] == "RBAC" {
                            30_000
                        } else {
                            15_000
                        },
                    ));
                cx.update_window(handle, |_, window, _| window.activate_window())
                    .unwrap();
                cx.run_until_parked();
            }
            if step["action"] == "completeArgoOperation" {
                backend.complete_argo_operation();
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(15_000));
                cx.run_until_parked();
            }
            if step["action"] == "replaySessions" {
                backend.replay_sessions(string(step, "script"));
                // The live-session query interval in `portForwardLifecycle.ts`.
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(3_000));
                cx.run_until_parked();
            }
            if step["action"] == "terminalOutput" {
                backend.terminal_output(string(step, "container"), string(step, "data"));
                cx.run_until_parked();
            }
            if step["action"] == "terminalFit" {
                cx.simulate_window_resize(
                    handle,
                    size(
                        px(number(step, "width") as f32),
                        px(number(step, "height") as f32),
                    ),
                );
                cx.run_until_parked();
            }
            println!("parity {id} step {index}: {}", string(step, "action"));
            fs::write(
                Path::new(&artifacts).join("gpui-progress.txt"),
                format!("{id} step {index}: {}\n", string(step, "action")),
            )
            .unwrap();
            // Reads a render starts must settle before the step observes or acts.
            cx.update_window(handle, |_, window, cx| window.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
            if step["action"] == "replayLiveData" {
                // Advance the production web refresh throttle, not wall-clock sleeps.
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(250));
                cx.run_until_parked();
            }
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
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
    fs::create_dir_all(&artifacts).unwrap();
    fs::write(
        report,
        serde_json::to_vec_pretty(&json!({"driver": "gpui", "scenarios": passed})).unwrap(),
    )
    .unwrap();
}
