//! Runs the shared scenarios in `e2e/parity/scenarios/` against the GPUI client
//! with the browser-mock fixture the web driver reads. Driven by
//! `bun scripts/e2e-parity.ts`, which exports the fixture and checks both reports.

#[path = "parity/adapters.rs"]
mod adapters;
#[path = "parity/argo.rs"]
mod argo;
#[path = "parity/argo_fields.rs"]
mod argo_fields;
#[path = "parity/code_coverage.rs"]
mod code_coverage;
mod common;
#[path = "parity/coverage.rs"]
mod coverage;
#[path = "parity/coverage_checks.rs"]
mod coverage_checks;
#[path = "parity/delete_review.rs"]
mod delete_review;
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
#[path = "parity/multi_context.rs"]
mod multi_context;
#[path = "parity/navigation.rs"]
mod navigation;
#[path = "parity/overview.rs"]
mod overview;
#[path = "parity/resource_filters.rs"]
mod resource_filters;
#[path = "parity/resource_table.rs"]
mod resource_table;
#[path = "parity/runner.rs"]
mod runner;
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
#[path = "parity/yaml_diagnostics.rs"]
mod yaml_diagnostics;
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
use runner::{failure_message, run_scenario};
use scroll::{reveal, seek};
use selection::{artifact_name, claim, scenarios, shard};
use serde_json::{Value, json};

fn string<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_else(|| panic!("missing {key}"))
}
fn number(v: &Value, key: &str) -> u64 {
    v[key].as_u64().unwrap_or_else(|| panic!("missing {key}"))
}
/// A row's element ID, or without a `context` the ID prefix that
/// `detail::resolve_resource` matches against every context's row.
fn row(step: &Value) -> String {
    let id = format!(
        "resource:{}:{}:{}",
        string(step, "kind"),
        string(step, "namespace"),
        string(step, "name")
    );
    match step["context"].as_str() {
        Some(context) => format!("{id}:{context}"),
        None => id,
    }
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
        || overview::act(window, cx, step)
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
        || multi_context::act(window, cx, step)
        || detail::act(window, cx, step)
        || detail_status::act(window, cx, step)
        || delete_review::act(window, cx, step)
        || details_metadata::act(window, cx, step)
        || lint_permissions::act(window, cx, step)
        || workspace::act(window, cx, step)
        || workspace_editing::act(window, cx, step)
        || feedback::act(window, cx, step)
        || incident_diagnostics::act(window, cx, step)
        || diagnostics::act(window, cx, step)
        || topology_spike::act(window, cx, step)
        || delivery::act(window, cx, step)
        || yaml_folds::act(window, cx, step)
        || yaml_diagnostics::act(window, cx, step))
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
    terminal::finish(window, cx, step);
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
