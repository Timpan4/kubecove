use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size};
use kubecove_gpui::{
    Inspector,
    backend::{Backend, Read, Scope, ScopeResult},
};
use kubecove_lib::models::{
    AppError, AppErrorKind, ClusterContext, NamespaceSummary, ResourceDetailsFull, ResourceSummary,
    ResourceTopology,
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
