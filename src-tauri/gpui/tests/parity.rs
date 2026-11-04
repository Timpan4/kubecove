//! Runs the shared scenarios in `e2e/parity/scenarios/` against the GPUI client
//! with the browser-mock fixture the web driver reads. Driven by
//! `bun scripts/e2e-parity.ts`, which exports the fixture and checks both reports.

mod common;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use common::Fixture;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::Inspector;
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
    match string(step, "action") {
        "openResources" => {
            window.click("open-resources", cx);
            window.render_frame(cx);
            assert!(window.find("resources").visible());
        }
        "search" => {
            window.click("search", cx);
            window.press("ctrl-a", cx);
            window.input(string(step, "value"), cx);
        }
        // Expectations on rows mean rendered in the table, possibly scrolled out of
        // view, as Playwright's `visible` state does.
        "expectResource" => assert!(window.try_find(row(step)).is_some()),
        "openResource" => window.click(row(step), cx),
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
        action => panic!("unknown parity action: {action}"),
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
    let backend = Arc::new(Fixture::load(Path::new(&fixture)));
    let scenarios = scenarios();
    assert!(!scenarios.is_empty());
    let mut passed = Vec::new();
    for scenario in &scenarios {
        let id = string(scenario, "id");
        cx.update(gpui_kit::init);
        let backend = backend.clone();
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
                |window, cx| cx.new(|cx| Inspector::new(backend, window, cx)),
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
