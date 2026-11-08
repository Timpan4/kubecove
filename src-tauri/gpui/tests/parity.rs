//! Runs the shared scenarios in `e2e/parity/scenarios/` against the GPUI client
//! with the browser-mock fixture the web driver reads. Driven by
//! `bun scripts/e2e-parity.ts`, which exports the fixture and checks both reports.

#[path = "parity/adapters.rs"]
mod adapters;
#[path = "parity/argo.rs"]
mod argo;
#[path = "parity/code_coverage.rs"]
mod code_coverage;
mod common;
#[path = "parity/coverage.rs"]
mod coverage;
#[path = "parity/coverage_checks.rs"]
mod coverage_checks;
#[path = "parity/delivery.rs"]
mod delivery;
#[path = "parity/detail.rs"]
mod detail;
#[path = "parity/detail_scroll.rs"]
mod detail_scroll;
#[path = "parity/detail_status.rs"]
mod detail_status;
#[path = "parity/details_metadata.rs"]
mod details_metadata;
#[path = "parity/diagnostics.rs"]
mod diagnostics;
#[path = "parity/feedback.rs"]
mod feedback;
#[path = "parity/incident_diagnostics.rs"]
mod incident_diagnostics;
#[path = "parity/lint_permissions.rs"]
mod lint_permissions;
#[path = "parity/live_data.rs"]
mod live_data;
#[path = "parity/live_sessions.rs"]
mod live_sessions;
#[path = "parity/locate.rs"]
mod locate;
#[path = "parity/log_coverage.rs"]
mod log_coverage;
#[path = "parity/log_restore.rs"]
mod log_restore;
#[path = "parity/log_toolbar.rs"]
mod log_toolbar;
#[path = "parity/map_gestures.rs"]
mod map_gestures;
#[path = "parity/map_panels.rs"]
mod map_panels;
#[path = "parity/mcp.rs"]
mod mcp;
#[path = "parity/navigation.rs"]
mod navigation;
#[path = "parity/resource_filters.rs"]
mod resource_filters;
#[path = "parity/resource_table.rs"]
mod resource_table;
#[path = "parity/scroll.rs"]
mod scroll;
#[path = "parity/selection.rs"]
mod selection;
#[path = "parity/session_health.rs"]
mod session_health;
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
#[path = "parity/topology_spike.rs"]
mod topology_spike;
#[path = "parity/workspace.rs"]
mod workspace;
#[path = "parity/workspace_editing.rs"]
mod workspace_editing;
#[path = "parity/yaml_folds.rs"]
mod yaml_folds;

use std::{
    any::Any, ffi::OsString, fs, panic::AssertUnwindSafe, path::Path, sync::Arc, time::Instant,
};

use common::Fixture;
use detail::reveal_resource;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size,
};
use kubecove_gpui::Inspector;
use scroll::{reveal, seek};
use selection::{artifact_name, claim, scenarios, shard};
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
/// The RBAC page's scrolling content, below the fixed workspace header.
const RBAC_TOP: &str = "page";

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

fn act(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) {
    if !(shell::act(window, cx, step)
        || coverage_checks::act(window, cx, step)
        || coverage::act(window, cx, step)
        || log_toolbar::act(window, cx, step)
        || argo::act(window, cx, step)
        || adapters::act(window, cx, step)
        || mcp::act(window, cx, step)
        || live_sessions::apply(window, cx, step)
        || log_restore::apply(window, cx, step)
        || session_health::act(window, cx, step)
        || terminal::apply(window, cx, step)
        || settings_integrations::act(window, cx, step)
        || live_data::apply(window, cx, step)
        || sidebar_live::apply(window, cx, step)
        || navigation::apply(window, cx, step)
        || resource_filters::act(window, cx, step)
        || map_panels::apply(window, cx, step)
        || topology_metrics::apply(window, cx, step)
        || resource_table::act(window, cx, step)
        || detail::act(window, cx, step)
        || detail_status::act(window, cx, step)
        || details_metadata::act(window, cx, step)
        || lint_permissions::act(window, cx, step)
        || workspace::act(window, cx, step)
        || workspace_editing::act(window, cx, step)
        || feedback::act(window, cx, step)
        || incident_diagnostics::act(window, cx, step)
        || diagnostics::act(window, cx, step)
        || topology_spike::act(window, cx, step)
        || delivery::act(window, cx, step)
        || yaml_folds::act(window, cx, step))
    {
        panic!("unknown parity action: {}", string(step, "action"));
    }
}

/// Finish inputs after the prior update dispatches its events.
/// Typing can update buttons; opening a picker can move keyboard focus.
fn finish(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App, step: &Value) {
    if step["action"] == "coverageSelect" {
        coverage::finish(window, cx, step);
    }
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
        reveal(window, "detail-body", "argo-sync-confirm", cx);
        window.click("argo-sync-confirm", cx);
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
    let report = Path::new(&artifacts).join(artifact_name("gpui", "json"));
    fs::create_dir_all(&artifacts).unwrap();
    match fs::remove_file(&report) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("cannot remove stale parity report: {error}"),
    }
    let scenarios = scenarios();
    // A shard can be empty when there are fewer selected scenarios than shards.
    assert!(shard().is_some() || !scenarios.is_empty());
    let mut results = Vec::new();
    for (index, scenario) in scenarios.iter().enumerate() {
        if !claim(&artifacts, index) {
            continue;
        }
        let id = string(scenario, "id");
        let started = Instant::now();
        cx.update(gpui_kit::init);
        // As the web driver does, record a failing scenario and run the rest.
        let mut window = None;
        code_coverage::reset();
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
            run_scenario(cx, scenario, &artifacts, &fixture, &mut window)
        }));
        code_coverage::write(&artifacts, index);
        let duration = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match outcome {
            Ok(()) => results.push(json!({"id": id, "status": "passed", "durationMs": duration})),
            Err(payload) => {
                // Close the failed scenario's window so it cannot affect the next one.
                if let Some(open) = window.take() {
                    let _ = cx.update_window(open, |_, window, _| window.remove_window());
                    cx.run_until_parked();
                }
                results.push(
                    json!({"id": id, "status": "failed", "durationMs": duration, "error": failure_message(&artifacts, payload)}),
                );
            }
        }
    }
    fs::create_dir_all(&artifacts).unwrap();
    fs::write(
        report,
        serde_json::to_vec_pretty(&json!({"driver": "gpui", "scenarios": results})).unwrap(),
    )
    .unwrap();
    let failed = results
        .iter()
        .filter(|result| result["status"] == "failed")
        .count();
    assert_eq!(failed, 0, "{failed} parity scenarios failed; see gpui.json");
}

/// Runs a step's checks in the window and re-raises their panic only after the update ends.
/// A panic that unwinds through `update_window` skips GPUI's end-of-update bookkeeping, so
/// effects stop flushing for the rest of the process and later scenarios fail on missing
/// elements.
fn check_in_window(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    check: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App),
) {
    let outcome = cx
        .update_window(handle, |_, window, cx| {
            std::panic::catch_unwind(AssertUnwindSafe(|| check(window, cx)))
        })
        .unwrap();
    if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
}

/// The panic text, plus the last step the driver reached.
fn failure_message(artifacts: &OsString, payload: Box<dyn Any + Send>) -> String {
    let panic = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .unwrap_or_else(|| "panic without a message".to_owned());
    let last_step =
        fs::read_to_string(Path::new(artifacts).join("gpui-progress.txt")).unwrap_or_default();
    format!("{panic} (last step: {})", last_step.trim())
}

fn run_scenario(
    cx: &mut TestAppContext,
    scenario: &Value,
    artifacts: &OsString,
    fixture: &OsString,
    window: &mut Option<AnyWindowHandle>,
) {
    let id = string(scenario, "id");
    // A fresh fixture per scenario, as the web driver reloads the page and its
    // in-memory mock state.
    let backend = Arc::new(Fixture::load(Path::new(fixture)));
    let steps = scenario["steps"].as_array().unwrap();
    // A scenario that starts without a workspace leaves the launcher as the startup view.
    let seeded = steps
        .first()
        .is_none_or(|step| step["action"] != "startWithoutWorkspace");
    // Leading phase and hold steps apply before the client starts, as the
    // web driver's first load does for shell reads.
    let startup_steps = steps
        .iter()
        .take_while(|step| {
            step["action"] == "configureShellRead"
                || (step["action"] == "mockTiming" && step["control"]["operation"] == "hold")
        })
        .count();
    for step in &steps[..startup_steps] {
        if step["action"] == "configureShellRead" {
            *backend.shell_read_phase.lock().unwrap() = string(step, "phase").to_owned();
        } else {
            backend.timing_control(&step["control"]);
        }
    }
    let initial_coverage = scenario["steps"]
        .as_array()
        .unwrap()
        .first()
        .filter(|step| {
            step["action"] == "coverageScript"
                && matches!(string(step, "script"), "loading" | "startup-failed")
        });
    if let Some(step) = initial_coverage {
        backend.configure_coverage(string(step, "script"));
    }
    let (opened, _) = cx.update(|cx| {
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
                    let inspector = Inspector::new(backend.clone(), window, cx);
                    if seeded {
                        inspector.with_saved_workspace("Parity mock workspace", "mock-dev", cx)
                    } else {
                        inspector
                    }
                })
            },
        )
        .unwrap()
    });
    // Recorded so a failing scenario can close its window before the next one opens.
    *window = Some(opened);
    let mut handle = opened;
    cx.run_until_parked();
    for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
        if index < startup_steps {
            continue;
        }
        if (step["action"] == "expectArgoText"
            && matches!(
                step["text"].as_str(),
                Some("Fixture discovery unavailable" | "Fixture provider detection unavailable")
            ))
            || (step["action"] == "expectPageText"
                && step["text"] == "Scripted page refresh failure")
            || (step["action"] == "expectLiveState" && step["label"] == "Refresh failed")
            || (step["action"] == "coverageText"
                && matches!(
                    step["text"].as_str(),
                    Some(
                        "Helm details unavailable"
                            | "Helm releases unavailable"
                            | "Incidents unavailable"
                            | "Kubeconfig sources failed"
                            | "Live sessions unavailable"
                            | "RBAC inspection unavailable"
                            | "Failed to load ownership map"
                    )
                ))
            || step["action"] == "expectLauncherRetry"
            || step["action"] == "expectFriendlyError"
            || step["action"] == "expectOverviewRetry"
            || step["action"] == "expectOverviewHealth"
            || step["action"] == "expectWorkspaceReadWarning"
        {
            for millis in [1000, 2000] {
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(millis));
                cx.run_until_parked();
            }
        }
        if step["action"] == "mockTiming" {
            // A cancelled kind discovery read retries after the first retry delay.
            if step["control"]["operation"] == "expectRequests"
                || (step["control"]["operation"] == "expectNoError"
                    && step["control"]["command"] == "list_resource_kinds")
            {
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(1000));
                cx.run_until_parked();
            }
            if step["control"]["operation"] == "expectNoRetry" {
                // The web's default maximum retry delay in query-core/src/retryer.ts.
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(
                        if step["control"]["command"] == "get_backend_diagnostics" {
                            1000
                        } else {
                            30_000
                        },
                    ));
                cx.run_until_parked();
            }
            if step["control"]["operation"] == "expectNoError" {
                check_in_window(cx, handle, |window, cx| {
                    window.render_frame(cx);
                    for id in [
                        "surface-error",
                        "load-error",
                        "gitops-error",
                        "argo-discovery-error",
                    ] {
                        assert!(
                            window.try_find(id).is_none(),
                            "Read error shown while retry is pending"
                        );
                    }
                });
            }
            backend.timing_control(&step["control"]);
            if step["control"]["operation"] == "contend" {
                for read in backend.competing_reads(&step["control"]) {
                    cx.background_executor
                        .spawn(async move {
                            let _ = read.await;
                        })
                        .detach();
                }
            }
            cx.run_until_parked();
            if step["control"]["operation"] == "release"
                && step["control"]["command"] == "list_resource_topology"
            {
                // read_retry.rs uses TanStack's first cancelled-read backoff.
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(1000));
                cx.run_until_parked();
            }
            if step["control"]["operation"] == "release"
                && step["control"]["command"] == "start_resource_watch"
            {
                // live-view-refresh.ts coalesces changes for 250 ms.
                cx.background_executor
                    .advance_clock(std::time::Duration::from_millis(250));
                cx.run_until_parked();
            }
        }
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
        if step["action"] == "coverageScript" && (index != 0 || initial_coverage.is_none()) {
            backend.configure_coverage(string(step, "script"));
        }
        if step["action"] == "coverageCheck" && step["check"] == "emit-log" {
            backend.emit_coverage_log();
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
            let preferences = serde_json::to_string(&backend.load_preferences().unwrap()).unwrap();
            assert!(
                !preferences.contains("parity-mcp"),
                "MCP credentials must not enter preferences"
            );
        }
        if step["action"] == "expectDocumentationRequest" {
            assert_eq!(backend.documentation_requests(), vec![string(step, "url")]);
        }
        if step["action"] == "restartClient"
            || (step["action"] == "configureShellRead" && step["phase"] != "ready")
        {
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
                            Inspector::new(backend.clone(), window, cx).with_saved_workspace(
                                "Parity mock workspace",
                                "mock-dev",
                                cx,
                            )
                        })
                    },
                )
                .unwrap()
            });

            handle = reopened;
            *window = Some(reopened);
            cx.run_until_parked();
        }
        if step["action"] == "reopenStoredSettings" {
            backend.reopen_settings_storage();
        }
        if step["action"] == "configureSettingsMock" {
            backend.settings_script(string(step, "script"));
        }
        if step["action"] == "dropRemoteConnection" {
            backend.drop_remote_connection();
        }
        if step["action"] == "remoteSessionSignal" {
            backend.remote_signal(serde_json::from_value(step["state"].clone()).unwrap());
        }
        if step["action"] == "expectRuntimeEndpoint" {
            assert_eq!(backend.runtime_endpoint(), string(step, "value"));
        }
        if step["action"] == "expectWorkspaceExport" {
            backend.expect_workspace_export(string(step, "name"));
        }
        if step["action"] == "expectExportedShortcut" {
            backend.expect_exported_shortcut(string(step, "name"), &step["shortcut"]);
        }
        if step["action"] == "replayLiveData" {
            backend.replay_live(string(step, "script"));
        }
        if step["action"] == "expectWatchStart" {
            backend.expect_watch_start(string(step, "kind"), step["resourceVersion"].as_str());
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
        if step["action"] == "resizeViewport" {
            cx.simulate_window_resize(
                handle,
                size(
                    px(number(step, "width") as f32),
                    px(number(step, "height") as f32),
                ),
            );
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
            Path::new(&artifacts).join(artifact_name("gpui-progress", "txt")),
            format!("{id} step {index}: {}\n", string(step, "action")),
        )
        .unwrap();
        // Reads a render starts must settle before the step observes or acts.
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
        if step["action"] == "advanceClock" {
            // The tracker's tick updates the window, so advance outside the window update below.
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(number(step, "ms")));
            cx.run_until_parked();
        }
        if step["action"] == "replayLiveData" {
            // Advance the production web refresh throttle, not wall-clock sleeps.
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(250));
            cx.run_until_parked();
        }
        check_in_window(cx, handle, |window, cx| {
            window.render_frame(cx);
            act(window, cx, step);
        });
        cx.run_until_parked();
        check_in_window(cx, handle, |window, cx| {
            window.render_frame(cx);
            finish(window, cx, step);
        });
        cx.run_until_parked();
        if matches!(string(step, "action"), "editYaml" | "replaceYamlDraft") {
            // The editor lints a changed draft after its 450 ms delay.
            cx.background_executor
                .advance_clock(std::time::Duration::from_millis(450));
            cx.run_until_parked();
        }
    }
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}
