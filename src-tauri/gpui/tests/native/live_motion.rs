//! Motion this client adds beyond the web: the tint on table rows a watch
//! notice added or changed, and the fade of content that replaces a loading
//! indicator. The shared `loading-motion.` parity scenarios cover that both
//! come to rest. The web has neither, so only native tests can stop inside
//! them. Failure modes, written before the code:
//!
//! - a changed or an added row gets no tint, or an unchanged row gets one;
//! - the tint starts below the web sidebar's 30 %, or does not ease out;
//! - the tint, or its frame requests, outlast one second;
//! - a reload, which names no rows, tints the rows it changed, whether the
//!   user asked for it or a lost watch recovered with it;
//! - reduced motion still tints, or a highlight that had started keeps fading
//!   after the preference turns on;
//! - a table that stopped drawing keeps its highlights and its frame requests;
//! - content fades in after a read that settled within the show delay, so
//!   every fast page change flickers;
//! - content after a shown indicator appears at once, or its fade never ends;
//! - reduced motion still fades.
use super::*;
use kubecove_lib::models::StreamMessage;
use std::time::Duration;

const DEPLOYMENT: &str = "resource:Deployment:payments:payments-api:mock-dev";
const POD: &str = "resource:Pod:payments:payments-api-pod:mock-dev";
/// `sidebar-row-reveal` starts at primary 30 %.
const TINT: f32 = 0.3;
const TOLERANCE: f32 = 1e-4;

/// The rows the next resource read returns, when a test scripts them.
#[derive(Default)]
pub(super) struct RowControls(Mutex<Option<Rows>>);

#[derive(Clone, Copy)]
enum Rows {
    /// A Deployment and its ReplicaSet.
    Before,
    /// The Deployment is degraded and a Pod exists. The ReplicaSet is as before.
    After,
}

impl RowControls {
    fn show(&self, rows: Rows) {
        *self.0.lock().unwrap() = Some(rows);
    }

    pub(super) fn read(&self, context: &str) -> Option<Vec<ResourceSummary>> {
        let rows = (*self.0.lock().unwrap())?;
        let row = |kind: &str, name: &str| -> ResourceSummary {
            serde_json::from_value(json!({
                "kind": kind, "cluster": context, "name": name,
                "namespace": "payments", "age": "1d"
            }))
            .unwrap()
        };
        let mut deployment = resource(context);
        let mut all = vec![row("ReplicaSet", "payments-api-rs")];
        if matches!(rows, Rows::After) {
            deployment.health_assessment.state =
                kubecove_lib::models::HealthAssessmentState::Degraded;
            all.push(row("Pod", "payments-api-pod"));
        }
        all.push(deployment);
        Some(all)
    }
}

fn frame(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn advance(cx: &mut TestAppContext, by: Duration) {
    cx.background_executor.advance_clock(by);
    cx.run_until_parked();
}

fn setup(cx: &mut TestAppContext) -> (Arc<Controlled>, gpui_kit::AnyWindowHandle) {
    let backend = Arc::new(Controlled::default());
    backend.rows.show(Rows::Before);
    let handle = open(cx, backend.clone());
    frame(cx, handle);
    (backend, handle)
}

/// A watch notice for the Deployment and the Pod, read and drawn. Each watch
/// reads its first notice at once, so the clock stays where it is.
fn update(cx: &mut TestAppContext, backend: &Controlled, handle: gpui_kit::AnyWindowHandle) {
    backend.rows.show(Rows::After);
    backend.live.change("Deployment", "payments-api");
    backend.live.change("Pod", "payments-api-pod");
    frame(cx, handle);
}

fn inspector(window: &gpui_kit::Window, cx: &gpui_kit::App) -> gpui_kit::Entity<Inspector> {
    gpui_kit::base::Root::read(window, cx)
        .view()
        .clone()
        .downcast::<Inspector>()
        .expect("the Inspector is the window's content")
}

/// The tinted rows with their tint, and whether the frame that drew them asked
/// for another.
fn highlights(
    cx: &mut TestAppContext,
    handle: gpui_kit::AnyWindowHandle,
) -> (Vec<(String, f32)>, bool) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        let rows = inspector(window, cx).read(cx).row_highlights();
        (rows, window.simulate_next_frame(cx) > 0)
    })
    .unwrap()
}

fn expect_degraded(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(DEPLOYMENT).description(), Some("Degraded"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_watch_update_tints_the_changed_and_the_added_row_for_one_second(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    assert_eq!(highlights(cx, handle), (vec![], false));

    update(cx, &backend, handle);
    expect_degraded(cx, handle);
    let (rows, moving) = highlights(cx, handle);
    let ids: Vec<_> = rows.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        [DEPLOYMENT, POD],
        "the unchanged ReplicaSet has no tint"
    );
    for (id, tint) in &rows {
        assert!((tint - TINT).abs() < TOLERANCE, "{id} starts at {tint}");
    }
    assert!(moving);

    advance(cx, Duration::from_millis(500));
    let (rows, moving) = highlights(cx, handle);
    assert_eq!(rows.len(), 2);
    for (id, tint) in &rows {
        // `ease-out` has faded more than half the tint at half its time.
        assert!(*tint > 0. && *tint < TINT / 2., "{id} shows {tint}");
    }
    assert!(moving);

    advance(cx, Duration::from_millis(500));
    assert_eq!(highlights(cx, handle), (vec![], false));
}

#[gpui_kit::test]
fn a_reload_tints_no_rows(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    backend.rows.show(Rows::After);
    refresh(cx, handle);
    frame(cx, handle);
    expect_degraded(cx, handle);
    assert_eq!(highlights(cx, handle).0, vec![]);
}

#[gpui_kit::test]
fn a_recovery_reload_tints_no_rows(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    backend.rows.show(Rows::After);
    backend.live.send(StreamMessage::Stopped {
        stream_id: "native".into(),
    });
    cx.run_until_parked();
    // resource-watch.ts::RESOURCE_RECOVERY_INTERVAL: the lost watch lists its kind.
    advance(cx, Duration::from_millis(15_000));
    frame(cx, handle);
    expect_degraded(cx, handle);
    assert_eq!(highlights(cx, handle).0, vec![]);
}

#[gpui_kit::test]
fn reduced_motion_tints_no_rows(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    cx.update(|cx| cx.set_reduce_motion(true));
    update(cx, &backend, handle);
    expect_degraded(cx, handle);
    assert_eq!(highlights(cx, handle), (vec![], false));
}

#[gpui_kit::test]
fn turning_reduced_motion_on_ends_a_running_highlight(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    update(cx, &backend, handle);
    assert_eq!(highlights(cx, handle).0.len(), 2);
    cx.update(|cx| cx.set_reduce_motion(true));
    assert_eq!(highlights(cx, handle), (vec![], false));
}

#[gpui_kit::test]
fn leaving_the_table_drops_its_highlights(cx: &mut TestAppContext) {
    let (backend, handle) = setup(cx);
    update(cx, &backend, handle);
    assert_eq!(highlights(cx, handle).0.len(), 2);
    cx.update_window(handle, |_, window, cx| {
        window.click("nav:GitOps", cx);
    })
    .unwrap();
    frame(cx, handle);
    // The click eases the navigation's controls for `control_motion::DURATION`.
    // A kept highlight would ask for frames for the rest of its second.
    advance(cx, kubecove_gpui::control_motion::DURATION);
    assert_eq!(highlights(cx, handle), (vec![], false));
}

/// Starts a resource read for the other context that stays pending until the
/// returned sender fires.
fn pending_read(
    cx: &mut TestAppContext,
    backend: &Controlled,
    handle: gpui_kit::AnyWindowHandle,
) -> tokio::sync::oneshot::Sender<()> {
    let (send, receive) = tokio::sync::oneshot::channel();
    *backend.pending.lock().unwrap() = Some(receive);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.within("context").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    send
}

/// The opacity of the Resources content while it fades in, and whether the
/// frame that drew it asked for another.
fn reveal(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) -> (Option<f32>, bool) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        let opacity = inspector(window, cx).read(cx).content_reveal("resources");
        (opacity, window.simulate_next_frame(cx) > 0)
    })
    .unwrap()
}

#[gpui_kit::test]
fn content_fades_in_after_a_shown_indicator(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let send = pending_read(cx, &backend, handle);
    // The show delay in `loading.rs`, then its minimum visible time.
    advance(cx, Duration::from_millis(300));
    assert_eq!(reveal(cx, handle).0, None, "the indicator shows");
    send.send(()).unwrap();
    advance(cx, Duration::from_millis(300));

    let (opacity, moving) = reveal(cx, handle);
    assert_eq!(opacity, Some(0.), "the fade starts");
    assert!(moving);

    advance(cx, Duration::from_millis(75));
    let (opacity, moving) = reveal(cx, handle);
    let opacity = opacity.expect("the content fades for 150 ms");
    // `ease` is past half its way at half its time.
    assert!(
        opacity > 0.5 && opacity < 1.,
        "half the fade drew {opacity}"
    );
    assert!(moving);

    advance(cx, Duration::from_millis(75));
    assert_eq!(reveal(cx, handle), (None, false));
}

#[gpui_kit::test]
fn content_after_a_fast_read_appears_at_once(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    let send = pending_read(cx, &backend, handle);
    advance(cx, Duration::from_millis(100));
    assert_eq!(reveal(cx, handle).0, None);
    send.send(()).unwrap();
    cx.run_until_parked();
    assert_eq!(reveal(cx, handle), (None, false));
}

#[gpui_kit::test]
fn reduced_motion_shows_content_at_once(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let handle = open(cx, backend.clone());
    cx.update(|cx| cx.set_reduce_motion(true));
    let send = pending_read(cx, &backend, handle);
    advance(cx, Duration::from_millis(300));
    send.send(()).unwrap();
    advance(cx, Duration::from_millis(300));
    assert_eq!(reveal(cx, handle), (None, false));
}
