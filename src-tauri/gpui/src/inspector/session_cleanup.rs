//! Live-session cleanup when the open workspace is deleted. It mirrors the cleanup effect in
//! `App.svelte`: with no workspace allowed, every forward and exec session stops unless the
//! keep-sessions preference is on. The outcome shows as a notice that stays until dismissed.
use super::{
    Inspector,
    controls::{Variant, button},
};
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::{AnyElement, Context, Task, TestSupportExt, div, prelude::*, px};
use kubecove_lib::models::{LiveSessionCleanupRequest, LiveSessionCleanupResult};

#[derive(Default)]
pub(super) struct SessionCleanup {
    /// The stop summary or failure from the last cleanup. It stays until dismissed.
    notice: Option<String>,
    task: Option<Task<()>>,
}

fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}

/// The web's notice text for a cleanup that stopped at least one session.
fn stopped_notice(result: &LiveSessionCleanupResult) -> Option<String> {
    let forwards = result.stopped_port_forwards;
    let exec = result.stopped_pod_exec_sessions;
    (forwards + exec > 0).then(|| {
        format!(
            "Stopped {forwards} port {} and {exec} exec {} outside this workspace.",
            plural(forwards, "forward", "forwards"),
            plural(exec, "session", "sessions"),
        )
    })
}

impl Inspector {
    /// Stops every live session after the open workspace is deleted. The read of the kubeconfig
    /// source runs first, so the stop uses the same source as the sessions it cleans.
    pub(super) fn stop_sessions_after_workspace_delete(&mut self, cx: &mut Context<Self>) {
        if self
            .persisted
            .preferences
            .keep_live_sessions_on_workspace_switch
        {
            return;
        }
        let backend = self.backend.clone();
        self.session_cleanup.task = Some(cx.spawn(async move |this, cx| {
            let outcome = async {
                let source = backend.kubeconfig_sources().await?.source_key;
                backend
                    .cleanup_sessions(LiveSessionCleanupRequest {
                        allowed_cluster_contexts: vec![],
                        kubeconfig_source_key: source,
                    })
                    .await
            }
            .await;
            let notice = match outcome {
                Ok(result) => stopped_notice(&result),
                Err(error) => Some(format!("Live session cleanup failed: {}", error.message)),
            };
            let _ = this.update(cx, |this, cx| {
                if let Some(notice) = notice {
                    this.session_cleanup.notice = Some(notice);
                }
                this.load_sessions(cx);
                cx.notify();
            });
        }));
    }

    /// The `Live sessions updated` alert above the workspace, with Dismiss.
    pub(super) fn session_cleanup_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let notice = self.session_cleanup.notice.clone()?;
        Some(
            div()
                .id("live-session-notice")
                .test_support()
                .role(gpui_kit::Role::Alert)
                .aria_label(notice.clone())
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .px(px(16.))
                .py(px(8.))
                .border_b_1()
                .border_color(Tokens::border())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(ui::medium(ui::xs("Live sessions updated")))
                        .child(ui::xs(notice)),
                )
                .child(
                    button(Variant::Ghost, 28., false)
                        .id("live-session-notice-dismiss")
                        .test_support()
                        .aria_label("Dismiss")
                        .child(ui::xs("Dismiss"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.session_cleanup.notice = None;
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}
