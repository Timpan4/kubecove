//! Failure modes: new identities never join the graph; watch bursts fan out
//! graph reads; a change during a read is lost; failed refreshes blank the map;
//! leaving the page keeps a delayed read alive. All waits use GPUI's test clock.
use super::*;
use kubecove_gpui::inspector::RESOURCE_WATCH_COALESCE;

const RS: &str = "payments-api-new";
const POD: &str = "payments-api-new-pod";

#[derive(Clone, Copy)]
pub(super) enum Snapshot {
    Deployment,
    ReplicaSet,
    Pod,
}

#[derive(Default)]
pub(super) struct MapControls(Mutex<Option<Snapshot>>);

impl MapControls {
    fn show(&self, snapshot: Snapshot) {
        *self.0.lock().unwrap() = Some(snapshot);
    }

    pub(super) fn rows(&self, context: &str) -> Option<Vec<ResourceSummary>> {
        let snapshot = (*self.0.lock().unwrap())?;
        let mut rows = vec![resource(context)];
        if !matches!(snapshot, Snapshot::Deployment) {
            rows.push(
                serde_json::from_value(json!({
                    "kind": "ReplicaSet", "cluster": context, "name": RS,
                    "namespace": "payments", "age": "<1m", "apiVersion": "apps/v1"
                }))
                .unwrap(),
            );
        }
        if matches!(snapshot, Snapshot::Pod) {
            rows.push(
                serde_json::from_value(json!({
                    "kind": "Pod", "cluster": context, "name": POD,
                    "namespace": "payments", "age": "<1m", "apiVersion": "v1"
                }))
                .unwrap(),
            );
        }
        Some(rows)
    }

    pub(super) fn topology(&self, context: &str) -> Option<Arc<ResourceTopology>> {
        let rows = self.rows(context)?;
        let ids: Vec<_> = rows
            .iter()
            .map(|row| {
                format!(
                    "{context}:{}:{}:payments:{}",
                    row.api_version.as_deref().unwrap_or("apps/v1"),
                    row.kind,
                    row.name
                )
            })
            .collect();
        let nodes: Vec<_> = rows
            .iter()
            .zip(&ids)
            .map(|(row, id)| {
                json!({
                    "id": id, "kind": row.kind, "name": row.name, "namespace": row.namespace,
                    "health": "healthy", "status": "Available", "selectable": true, "summary": row
                })
            })
            .collect();
        let edges: Vec<_> = ids
            .windows(2)
            .map(|pair| {
                json!({
                    "id": format!("{}->{}", pair[0], pair[1]), "source": pair[0],
                    "target": pair[1], "relation": "owns"
                })
            })
            .collect();
        Some(Arc::new(
            serde_json::from_value(json!({"nodes": nodes, "edges": edges, "warnings": []}))
                .unwrap(),
        ))
    }
}

fn setup(cx: &mut TestAppContext) -> (Arc<Controlled>, gpui_kit::AnyWindowHandle) {
    let backend = Arc::new(Controlled::default());
    backend.map_live.show(Snapshot::Deployment);
    let handle = open(cx, backend.clone());
    frame(cx, handle);
    (backend, handle)
}

fn frame(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn tick(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    frame(cx, handle);
    cx.background_executor
        .advance_clock(RESOURCE_WATCH_COALESCE);
    frame(cx, handle);
}

fn expect_map(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, counts: &str) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("map-counts").label(), Some(counts));
    })
    .unwrap();
}

#[gpui_kit::test]
fn additions_refresh_the_map_once_and_keep_the_graph_while_reading(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    let before = backend.retry.calls.load(Ordering::SeqCst);
    let (release, gate) = tokio::sync::oneshot::channel();
    *backend.retry.gate.lock().unwrap() = Some(gate);
    backend.map_live.show(Snapshot::Pod);
    backend.live.change("ReplicaSet", RS);
    backend.live.change("Pod", POD);
    tick(cx, handle);
    expect_map(cx, handle, "1 nodes / 0 edges");
    cx.update_window(handle, |_, window, cx| {
        for (kind, name) in [
            ("Deployment", "payments-api"),
            ("ReplicaSet", RS),
            ("Pod", POD),
        ] {
            assert!(
                window
                    .try_find(format!("resource:{kind}:payments:{name}:mock-dev"))
                    .is_some()
            );
        }
        window.render_frame(cx);
    })
    .unwrap();
    release.send(()).unwrap();
    frame(cx, handle);
    expect_map(cx, handle, "3 nodes / 2 edges");
    assert_eq!(backend.retry.calls.load(Ordering::SeqCst), before + 1);
}

#[gpui_kit::test]
fn a_change_during_a_graph_read_gets_a_followup_read(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    let (release, gate) = tokio::sync::oneshot::channel();
    *backend.retry.gate.lock().unwrap() = Some(gate);
    backend.map_live.show(Snapshot::ReplicaSet);
    backend.live.change("ReplicaSet", RS);
    tick(cx, handle);
    backend.map_live.show(Snapshot::Pod);
    backend.live.change("Pod", POD);
    frame(cx, handle);
    release.send(()).unwrap();
    tick(cx, handle);
    expect_map(cx, handle, "3 nodes / 2 edges");
}

#[gpui_kit::test]
fn a_failed_live_graph_refresh_keeps_the_graph_and_recovers_on_change(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    backend
        .retry
        .errors
        .lock()
        .unwrap()
        .push_back(AppErrorKind::Forbidden);
    backend.map_live.show(Snapshot::ReplicaSet);
    backend.live.change("ReplicaSet", RS);
    tick(cx, handle);
    expect_map(cx, handle, "1 nodes / 0 edges");
    cx.update_window(handle, |_, window, cx| {
        assert!(window.try_find("map-refresh-error").is_some());
        window.render_frame(cx);
    })
    .unwrap();
    backend.live.change("ReplicaSet", RS);
    tick(cx, handle);
    expect_map(cx, handle, "2 nodes / 1 edges");
    cx.update_window(handle, |_, window, _| {
        assert!(window.try_find("map-refresh-error").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn leaving_the_page_cancels_a_scheduled_live_graph_read(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    let before = backend.retry.calls.load(Ordering::SeqCst);
    backend.map_live.show(Snapshot::ReplicaSet);
    backend.live.change("ReplicaSet", RS);
    frame(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.click("nav:GitOps", cx);
    })
    .unwrap();
    tick(cx, handle);
    assert_eq!(backend.retry.calls.load(Ordering::SeqCst), before);
}
