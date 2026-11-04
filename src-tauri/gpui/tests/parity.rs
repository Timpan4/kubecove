use std::{fs, sync::Arc};

use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::{
    Inspector,
    backend::{Backend, LiveSessions, Read, Scope, ScopeResult},
    gitops::GitOpsInventory,
};
use kubecove_lib::models::{
    ClusterContext, IncidentCockpitSummary, NamespaceSummary, ResourceDetailsFull,
    ResourceEventSummary, ResourceSummary, ResourceTopology,
};
use serde_json::{Value, json};

struct Fixture;

fn resource(kind: &str) -> ResourceSummary {
    serde_json::from_value(json!({"kind":kind,"cluster":"mock-dev","name":"payments-api", "namespace":"payments", "age":"1d"})).unwrap()
}

impl Backend for Fixture {
    fn contexts(&self) -> Read<Vec<ClusterContext>> {
        Box::pin(async {
            Ok(vec![ClusterContext {
                name: "mock-dev".into(),
                is_current: true,
            }])
        })
    }
    fn resources(&self, scope: Scope, _: bool) -> Read<ScopeResult> {
        Box::pin(async move {
            let resources = ["Deployment", "Service", "Pod"]
                .into_iter()
                .filter(|_| {
                    scope
                        .namespace
                        .as_deref()
                        .is_none_or(|namespace| namespace == "payments")
                })
                .map(resource)
                .collect();
            Ok(ScopeResult {
                namespaces: vec![NamespaceSummary {
                    name: "payments".into(),
                    age: "1d".into(),
                    created_at: None,
                }],
                resources,
                warnings: vec![],
            })
        })
    }
    fn details(&self, summary: ResourceSummary) -> Read<ResourceDetailsFull> {
        Box::pin(async move {
            let yaml = format!(
                "apiVersion: v1\nkind: {}\nmetadata:\n  name: {}\n  namespace: payments\n",
                summary.kind, summary.name
            );
            Ok(ResourceDetailsFull {
                metadata: json!({"name":summary.name,"namespace":"payments"}),
                summary,
                yaml,
                status: None,
            })
        })
    }
    fn topology(&self, _: Scope) -> Read<ResourceTopology> {
        Box::pin(async {
            Ok(ResourceTopology {
                nodes: vec![],
                edges: vec![],
                warnings: vec![],
            })
        })
    }
    fn gitops(&self, _: String) -> Read<GitOpsInventory> {
        Box::pin(async { Ok(GitOpsInventory::default()) })
    }
    fn cluster_resources(&self, _: String, _: bool) -> Read<ScopeResult> {
        Box::pin(async {
            Ok(ScopeResult {
                namespaces: vec![],
                resources: vec![],
                warnings: vec![],
            })
        })
    }
    fn incidents(&self, cluster: String) -> Read<IncidentCockpitSummary> {
        Box::pin(async move {
            Ok(IncidentCockpitSummary {
                cluster,
                generated_at: String::new(),
                requested_scope: vec![],
                items: vec![],
                warnings: vec![],
            })
        })
    }
    fn events(&self, _: ResourceSummary, _: Option<String>) -> Read<Vec<ResourceEventSummary>> {
        Box::pin(async { Ok(vec![]) })
    }
    fn live_sessions(&self) -> Read<LiveSessions> {
        Box::pin(async {
            Ok(LiveSessions {
                port_forwards: vec![],
                exec_sessions: vec![],
                source_key: None,
                show_source_labels: false,
            })
        })
    }
}

fn string<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_else(|| panic!("missing {key}"))
}
fn row(step: &Value) -> String {
    format!(
        "resource:{}:{}:{}",
        string(step, "kind"),
        string(step, "namespace"),
        string(step, "name")
    )
}

#[gpui_kit::test]
fn shared_inspection_scenarios(cx: &mut TestAppContext) {
    let artifacts =
        std::env::var_os("KUBECOVE_E2E_ARTIFACTS").expect("KUBECOVE_E2E_ARTIFACTS is required");
    let report = std::path::Path::new(&artifacts).join("gpui.json");
    match fs::remove_file(&report) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("cannot remove stale parity report: {error}"),
    }
    let contract: Value =
        serde_json::from_str(include_str!("../../../e2e/parity/inspection.json")).unwrap();
    assert_eq!(contract["context"], "mock-dev");
    let scenarios = contract["scenarios"].as_array().unwrap();
    assert!(!scenarios.is_empty());
    let mut passed = Vec::new();
    for scenario in scenarios {
        cx.update(gpui_kit::init);
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1200.), px(800.)),
                    })),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Inspector::new(Arc::new(Fixture), window, cx)),
            )
            .unwrap()
        });
        cx.run_until_parked();
        for step in scenario["steps"].as_array().unwrap() {
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
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
                    "expectResource" => assert!(window.find(row(step)).visible()),
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
                    action => panic!("unknown parity action: {action}"),
                }
            })
            .unwrap();
            cx.run_until_parked();
        }
        passed.push(json!({"id": string(scenario,"id"), "status":"passed"}));
    }
    fs::create_dir_all(&artifacts).unwrap();
    fs::write(
        report,
        serde_json::to_vec_pretty(&json!({"driver":"gpui","scenarios":passed})).unwrap(),
    )
    .unwrap();
}
