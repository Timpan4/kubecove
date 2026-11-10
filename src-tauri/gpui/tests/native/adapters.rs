// Failure mode: disabling or clearing collection must discard pending timings.
use super::{Controlled, open};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext};
use kubecove_gpui::diagnostics::Timings;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub const SECRET_YAML: &str = "apiVersion: v1\nkind: Secret\nmetadata:\n  name: payments-api\n  namespace: payments\ndata:\n  token: REDACTED\n";
#[derive(Default)]
pub struct SecretControls {
    pub enabled: AtomicBool,
    pub pending: Mutex<Option<tokio::sync::oneshot::Receiver<String>>>,
}
impl SecretControls {
    pub fn reveal(&self) -> kubecove_gpui::backend::Read<String> {
        let pending = self.pending.lock().unwrap().take();
        Box::pin(async move {
            match pending {
                Some(receiver) => receiver.await.map_err(|_| {
                    kubecove_lib::models::AppError::new(
                        "Cancelled",
                        kubecove_lib::models::AppErrorKind::Cluster,
                    )
                }),
                None => Ok("c2VjcmV0".into()),
            }
        })
    }
}

fn secret_detail(cx: &mut TestAppContext, backend: Arc<Controlled>) -> gpui_kit::AnyWindowHandle {
    backend.secret.enabled.store(true, Ordering::SeqCst);
    let handle = open(cx, backend);
    for id in [
        "open-settings",
        "setting:Redact secrets",
        "settings-back",
        // Back lands on the Workspace Overview.
        "open-resources",
        "resource:Secret:payments:payments-api:mock-dev",
        "show-yaml",
    ] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    handle
}

#[gpui_kit::test]
fn closing_detail_cancels_pending_reveal(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.secret.pending.lock().unwrap() = Some(receiver);
    let handle = secret_detail(cx, backend);
    for id in ["secret-reveal:data:token", "close-detail"] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    assert!(
        sender.send("c2VjcmV0".into()).is_err(),
        "detail close must drop the pending reveal"
    );
}

#[gpui_kit::test]
fn retrying_a_failed_reveal_clears_its_error(cx: &mut TestAppContext) {
    let backend = Arc::new(Controlled::default());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    *backend.secret.pending.lock().unwrap() = Some(receiver);
    sender.send("not base64!".into()).unwrap();
    let handle = secret_detail(cx, backend);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("secret-reveal:data:token", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("secret-error:data:token").is_some());
        window.click("secret-reveal:data:token", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("secret-value:data:token").is_some());
        assert!(window.try_find("secret-error:data:token").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn revealed_values_expire_at_the_web_lifetime(cx: &mut TestAppContext) {
    let handle = secret_detail(cx, Arc::new(Controlled::default()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("secret-reveal:data:token", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("secret-value:data:token").is_some());
    })
    .unwrap();
    cx.background_executor
        .advance_clock(std::time::Duration::from_secs(60));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("secret-value:data:token").is_none());
    })
    .unwrap();
}

#[test]
fn timing_collection_obeys_enable_and_clear_epochs() {
    let timings = Timings::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let read = || {
        Box::pin(async { Ok::<_, kubecove_lib::models::AppError>(()) })
            as kubecove_gpui::backend::Read<()>
    };
    runtime
        .block_on(timings.measure("disabled", read()))
        .unwrap();
    assert!(timings.snapshot().is_empty());
    timings.set_enabled(true);
    runtime
        .block_on(timings.measure("get_resource_yaml", read()))
        .unwrap();
    assert_eq!(timings.snapshot()[0].name, "get_resource_yaml");
    assert!(timings.snapshot()[0].duration_ms >= 0.);
    let pending = timings.measure("cleared", read());
    timings.clear();
    runtime.block_on(pending).unwrap();
    assert!(timings.snapshot().is_empty());
    let pending = timings.measure("disabled-in-flight", read());
    timings.set_enabled(false);
    runtime.block_on(pending).unwrap();
    assert!(timings.snapshot().is_empty());
}
