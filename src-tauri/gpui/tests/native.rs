use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::{
    Inspector,
    backend::{Backend, LiveSessions, Read, Scope, ScopeResult},
    gitops::GitOpsInventory,
};
use kubecove_lib::models::{
    AppError, AppErrorKind, ClusterContext, IncidentCockpitSummary, NamespaceSummary,
    ResourceDetailsFull, ResourceEventSummary, ResourceSummary, ResourceTopology,
};
use serde_json::json;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
struct Controlled {
    fail: AtomicBool,
    pending: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

fn resource(context: &str) -> ResourceSummary {
    serde_json::from_value(json!({"kind":"Deployment","cluster":context,"name":"payments-api","namespace":"payments","age":"1d"})).unwrap()
}

impl Backend for Controlled {
    fn contexts(&self) -> Read<Vec<ClusterContext>> {
        Box::pin(async {
            Ok(vec![
                ClusterContext {
                    name: "mock-dev".into(),
                    is_current: true,
                },
                ClusterContext {
                    name: "mock-other".into(),
                    is_current: false,
                },
            ])
        })
    }
    fn resources(&self, scope: Scope, _: bool) -> Read<ScopeResult> {
        let fail = self.fail.load(Ordering::SeqCst);
        let pending = self.pending.lock().unwrap().take();
        Box::pin(async move {
            if let Some(pending) = pending {
                let _ = pending.await;
            }
            if fail {
                return Err(AppError::new(
                    "Resource read denied",
                    AppErrorKind::Forbidden,
                ));
            }
            Ok(ScopeResult {
                namespaces: vec![NamespaceSummary {
                    name: "payments".into(),
                    age: "1d".into(),
                    created_at: None,
                }],
                resources: vec![resource(&scope.context)],
                warnings: vec![],
            })
        })
    }
    fn details(&self, summary: ResourceSummary) -> Read<ResourceDetailsFull> {
        Box::pin(async move {
            Ok(ResourceDetailsFull {summary, yaml:"apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: payments-api\n  namespace: payments\n".into(),metadata:json!({"name":"payments-api","namespace":"payments"}),status:None})
        })
    }
    fn topology(&self, scope: Scope) -> Read<ResourceTopology> {
        Box::pin(async move {
            Ok(serde_json::from_value(json!({
                "nodes": [{"id": MAP_NODE, "kind": "Deployment", "name": "payments-api", "namespace": "payments", "status": "Available", "health": "healthy", "selectable": true, "summary": resource(&scope.context)}],
                "edges": [],
                "warnings": [],
            }))
            .unwrap())
        })
    }
    fn gitops(&self, _: String) -> Read<GitOpsInventory> {
        Box::pin(async { Ok(GitOpsInventory::default()) })
    }
    fn cluster_resources(&self, context: String, _: bool) -> Read<ScopeResult> {
        Box::pin(async move {
            Ok(ScopeResult {
                namespaces: vec![],
                resources: vec![serde_json::from_value(json!({"kind":"Node","cluster":context,"name":"dev-control-plane","namespace":null,"age":"9d"})).unwrap()],
                warnings: vec![],
            })
        })
    }
    fn events(&self, _: ResourceSummary, _: Option<String>) -> Read<Vec<ResourceEventSummary>> {
        Box::pin(async {
            Ok(serde_json::from_value(json!([
                {"eventType": "Normal", "reason": "Pulled", "message": "Image present.", "count": 1, "lastSeen": "8m", "lastSeenAt": "2026-06-29T09:52:00Z", "source": "kubelet", "namespace": "payments"},
                {"eventType": "Warning", "reason": "BackOff", "message": "Back-off restarting failed container app", "count": 4, "lastSeen": "2m", "lastSeenAt": "2026-06-29T09:58:00Z", "source": "kubelet", "namespace": "payments"},
            ]))
            .unwrap())
        })
    }
    fn incidents(&self, cluster: String) -> Read<IncidentCockpitSummary> {
        let item = |kind: &str, name: &str, severity: &str| {
            let status = if kind == "Pod" {
                "CrashLoopBackOff"
            } else {
                "Progressing"
            };
            json!({
                "resource": {"kind": kind, "cluster": cluster, "name": name, "namespace": "payments", "age": "1d", "status": status},
                "severity": severity, "state": "active", "warningEventCount": 0,
                "signals": [{"kind": kind, "label": format!("{name} signal"), "message": "needs attention", "source": "test", "state": "active"}],
            })
        };
        let summary = json!({
            "cluster": cluster, "generatedAt": "", "requestedScope": [], "warnings": [],
            "items": [item("Deployment", "payments-api", "attention"), item("Pod", "payments-api-0", "degraded")],
        });
        Box::pin(async move { Ok(serde_json::from_value(summary).unwrap()) })
    }
    fn live_sessions(&self) -> Read<LiveSessions> {
        let session = |id: &str, context: &str, source: &str| {
            json!({
                "id": id, "clusterContext": context, "kubeconfigSourceKey": source,
                "namespace": "payments", "targetKind": "Pod", "targetName": "payments-api-0",
                "podName": "payments-api-0", "remotePort": 8080, "resolvedPodName": "payments-api-0",
                "resolvedPodPort": 8080, "localPort": 18080, "localAddress": "127.0.0.1",
                "localUrl": "http://127.0.0.1:18080", "status": "listening", "startedAt": "2026-06-29T10:00:00Z",
            })
        };
        let forwards = [
            session("visible", "mock-dev", "kubeconfigSource=active"),
            session("other-source", "mock-dev", "kubeconfigSource=other"),
            session("other-context", "mock-other", "kubeconfigSource=active"),
        ];
        Box::pin(async move {
            Ok(LiveSessions {
                port_forwards: forwards
                    .into_iter()
                    .map(|forward| serde_json::from_value(forward).unwrap())
                    .collect(),
                exec_sessions: vec![],
                source_key: Some("kubeconfigSource=active".into()),
                show_source_labels: false,
            })
        })
    }
}

const MAP_NODE: &str = "mock-dev:apps/v1:Deployment:payments:payments-api";

fn open(cx: &mut TestAppContext, backend: Arc<Controlled>) -> gpui_kit::AnyWindowHandle {
    cx.update(gpui_kit::init);
    let (window, _) = cx.update(|cx| {
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
            |window, cx| cx.new(|cx| Inspector::new(backend, window, cx)),
        )
        .unwrap()
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-resources", cx);
    })
    .unwrap();
    cx.run_until_parked();
    window
}

/// Refresh lives in the Live updates menu, as in the web top bar.
fn refresh(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("live-status", cx);
        window.render_frame(cx);
        window.click("refresh", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_read_failure_is_visible_and_can_be_retried(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    backend.fail.store(true, Ordering::SeqCst);
    let handle = open(cx, backend.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("load-error").label(),
            Some("Resource read denied")
        );
        assert!(window.try_find("empty").is_none());
    })
    .unwrap();
    backend.fail.store(false, Ordering::SeqCst);
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("load-error").is_none());
        assert!(
            window
                .find("resource:Deployment:payments:payments-api")
                .visible()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_ownership_map_opens_resources(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        // The first frame measures the graph viewport; the second fits it.
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find("map-counts").label(), Some("1 nodes / 0 edges"));
        // An ownerless resource starts inside its collapsed kind group.
        assert!(window.try_find(format!("map-node:{MAP_NODE}")).is_none());
        window.click("map-group:Deployment", cx);
        window.render_frame(cx);
        window.click(format!("map-node:{MAP_NODE}"), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find("detail-context")
                .label()
                .unwrap()
                .contains("mock-dev")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn switching_context_cancels_the_old_read(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receive);
    refresh(cx, handle);
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        send.send(()).is_err(),
        "old read must release its receiver on scope change"
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find("scope-context")
                .label()
                .unwrap()
                .contains("mock-other")
        );
        window.click("resource:Deployment:payments:payments-api", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find("detail-context")
                .label()
                .unwrap()
                .contains("mock-other")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_local_draft_has_a_diff_and_requires_explicit_discard(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.click("resource:Deployment:payments:payments-api", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("show-yaml", cx);
        window.render_frame(cx);
        window.click("edit-draft", cx);
        window.render_frame(cx);
        let editor = window
            .find_by_label("Resource YAML")
            .path()
            .last()
            .unwrap()
            .clone();
        window.within("detail").click(editor, cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
        window.input(
            "kind: Deployment\nmetadata:\n  name: changed-local-draft\n",
            cx,
        );
        window.click("show-diff", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("draft-diff").visible());
        assert!(
            window
                .find("draft-notice")
                .label()
                .unwrap()
                .contains("local")
        );
        assert!(window.try_find("apply").is_none());
        window.click("close-detail", cx);
        window.render_frame(cx);
        assert!(
            window.find("close-detail").visible(),
            "a dirty draft blocks closing"
        );
        window.click("discard-draft", cx);
        window.render_frame(cx);
        window.click("close-detail", cx);
        assert!(window.try_find("detail-identity").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn warning_events_feed_the_incident_summary_and_timeline(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("resource:Deployment:payments:payments-api", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("incident-signal:Warning events").label(),
            Some("1 warning reason · 4 repeats")
        );
        // Only warnings reach the timeline; BackOff gets its presentation title.
        assert!(window.find("incident-timeline").visible());
    })
    .unwrap();
}

/// Failure mode: sessions from another context or kubeconfig source leak into
/// the page. The browser mocks hold sessions from one source only.
#[gpui_kit::test]
fn port_forwards_stay_in_the_workspace_scope(cx: &mut TestAppContext) {
    let handle = open(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        // The header counts every host session; the page filters to this workspace.
        assert_eq!(
            window.find("live-sessions").label(),
            Some("Open live sessions (3 active)")
        );
        window.click("live-sessions", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("forward:visible").label(),
            Some("Pod/payments-api-0:8080")
        );
        assert!(window.try_find("forward:other-source").is_none());
        assert!(window.try_find("forward:other-context").is_none());
    })
    .unwrap();
}
