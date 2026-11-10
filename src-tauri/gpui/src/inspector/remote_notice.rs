//! Session notices for a selected remote that is not connected, and their
//! actions (ADR 0031). Settings > Remote and the workspace content area show
//! the same notice.
use super::{
    Inspector, Page,
    controls::{Variant, button},
};
use crate::targets::TestSupportExt;
use crate::{backend::RemoteConnectionSummary, theme::Tokens, ui};
use gpui_kit::{ClipboardItem, Context, Div, SharedString, Window, div, prelude::*, px};
use kubecove_host::remote::{
    RemoteAuthMode, RemoteNotice, RemoteNoticeAction, RemoteSessionState, SIGN_IN_MEMORY_ONLY,
    SIGN_IN_NOT_DELETED,
};

/// Notes on how this computer keeps the sign-in, shown under a remote's
/// status or notice.
pub(super) fn sign_in_notes(
    remote: &RemoteConnectionSummary,
    prefix: &str,
) -> Vec<gpui_kit::AnyElement> {
    [
        (
            remote.sign_in_memory_only,
            "memory-only",
            SIGN_IN_MEMORY_ONLY,
        ),
        (
            remote.sign_in_not_deleted,
            "not-deleted",
            SIGN_IN_NOT_DELETED,
        ),
    ]
    .into_iter()
    .filter(|(shown, _, _)| *shown)
    .map(|(_, id, text)| {
        ui::muted(ui::xs(text))
            .id(SharedString::from(format!("{prefix}-{id}")))
            .test_support()
            .aria_label(text)
            .into_any_element()
    })
    .collect()
}

/// Where a notice renders. Each place has its own element ids, so parity
/// steps can click a Settings button or a content-area button.
#[derive(Clone, Copy)]
pub(super) enum NoticePlace {
    Settings,
    Content,
}

impl NoticePlace {
    /// The prefix of the notice's text ids.
    fn text(self) -> &'static str {
        match self {
            Self::Settings => "settings-notice",
            Self::Content => "remote-content-notice",
        }
    }
    /// The prefix of its button ids. Settings buttons share `settings-button:`.
    fn button(self) -> &'static str {
        match self {
            Self::Settings => "settings-button",
            Self::Content => "remote-content-notice-button",
        }
    }
}

impl Inspector {
    /// Starts the watch that reads the session state again on each session
    /// change, so a moved, expired or refused session shows its notice without
    /// opening Settings or waiting for a failed call.
    pub(super) fn sync_remote_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.integrations.session_watch.is_some() {
            return;
        }
        let mut changes = self.backend.remote_session_changes();
        let task = cx.spawn_in(window, async move |this, cx| {
            while changes.changed().await.is_ok() {
                let updated = this.update_in(cx, |this, window, cx| {
                    if this.settings.integrations.remote.is_some() {
                        this.refresh_remote_status(window, cx);
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        });
        self.settings.integrations.session_watch = Some(task);
    }

    fn refresh_remote_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let read = self.backend.remote_status();
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(remote) = result {
                    this.settings.integrations.remote = remote;
                }
                cx.notify();
            });
        });
        self.settings.integrations.status_read = Some(task.into());
    }

    pub(super) fn remote_notice_action(
        &mut self,
        action: RemoteNoticeAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            // A sign-in that opens a session resets the runtime, as Connect does.
            RemoteNoticeAction::SignIn => {
                if self.can_leave(cx) {
                    self.sign_in_remote(window, cx);
                }
            }
            RemoteNoticeAction::CopySignInLink => self.copy_sign_in_link(window, cx),
            RemoteNoticeAction::CancelSignIn => {
                // Detached, so no later status read or copy can drop the cancel.
                let read = self.backend.remote_cancel_sign_in();
                cx.spawn_in(window, async move |this, cx| {
                    let result = read.await;
                    let _ = this.update_in(cx, |this, window, cx| match result {
                        Ok(()) => this.refresh_remote_status(window, cx),
                        Err(error) => {
                            this.settings.integrations.remote_error = Some(error.message);
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            RemoteNoticeAction::StartNewSession => {
                if self.settings.integrations.remote_call.is_some() || !self.can_leave(cx) {
                    return;
                }
                let read = self.backend.remote_start_new_session();
                self.run_remote_change(read, window, cx);
            }
            RemoteNoticeAction::CancelConnecting | RemoteNoticeAction::Disconnect => {
                self.disconnect_settings_remote(window, cx);
            }
        }
    }

    /// Opens the browser sign-in. The host call returns when the sign-in
    /// finishes or is cancelled, so the notice shows the wait until then.
    fn sign_in_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.settings.integrations;
        let Some(remote) = state.remote.as_mut() else {
            return;
        };
        remote.session_state = RemoteSessionState::WaitingForBrowser;
        remote.connected = false;
        state.remote_error = None;
        let read = self.backend.remote_sign_in();
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settings.integrations.sign_in = None;
                match result {
                    Ok(summary) => this.apply_remote_summary(summary, window, cx),
                    Err(error) => {
                        this.settings.integrations.remote_error = Some(error.message);
                        this.refresh_remote_status(window, cx);
                    }
                }
                cx.notify();
            });
        });
        self.settings.integrations.sign_in = Some(task.into());
        cx.notify();
    }

    /// Detached, so it shares no slot with the status read it may overlap.
    fn copy_sign_in_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let read = self.backend.remote_sign_in_link();
        cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Some(link)) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(link));
                        this.show_copy_toast("Copied sign-in link.".into(), cx);
                    }
                    Ok(None) => {}
                    Err(error) => this.settings.integrations.remote_error = Some(error.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn sign_out_settings_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.integrations.remote_call.is_some() || !self.can_leave(cx) {
            return;
        }
        let read = self.backend.remote_sign_out();
        self.run_remote_change(read, window, cx);
    }

    /// Runs a call that returns the new summary, and resets the runtime when the
    /// session opens or closes.
    fn run_remote_change(
        &mut self,
        read: crate::backend::Read<RemoteConnectionSummary>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settings.integrations.remote_call = None;
                match result {
                    Ok(summary) => this.apply_remote_summary(Some(summary), window, cx),
                    Err(error) => this.settings.integrations.remote_error = Some(error.message),
                }
                cx.notify();
            });
        });
        self.start_remote_call(task, cx);
    }

    fn apply_remote_summary(
        &mut self,
        summary: Option<RemoteConnectionSummary>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_live = self
            .settings
            .integrations
            .remote
            .as_ref()
            .is_some_and(is_live);
        let live = summary.as_ref().is_some_and(is_live);
        self.settings.integrations.remote = summary;
        if live || was_live {
            self.runtime_changed(window, cx);
        }
    }

    /// The notice in place of cluster views while the selected remote is not
    /// connected. Settings shows its own copy.
    pub(super) fn remote_content_notice(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        if self.page == Page::Settings {
            return None;
        }
        let remote = self.settings.integrations.remote.as_ref()?;
        let notice = remote.notice()?;
        Some(
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .p(px(24.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .w(px(436.))
                        .p(px(16.))
                        .rounded(px(10.))
                        .border_1()
                        .border_color(Tokens::border())
                        .child(self.remote_notice(&notice, remote, NoticePlace::Content, cx)),
                )
                .into_any_element(),
        )
    }

    pub(super) fn remote_notice(
        &self,
        notice: &RemoteNotice,
        remote: &RemoteConnectionSummary,
        place: NoticePlace,
        cx: &mut Context<Self>,
    ) -> Div {
        let (prefix, buttons) = (place.text(), place.button());
        let busy = self.settings.integrations.remote_call.is_some();
        let mut body = div().flex().flex_col().gap(px(8.));
        if let Some(title) = notice.title {
            body = body.child(
                ui::medium(ui::sm(title))
                    .id(SharedString::from(format!("{prefix}-title")))
                    .test_support()
                    .aria_label(title),
            );
        }
        body = body.child(
            ui::sm(notice.message)
                .id(SharedString::from(format!("{prefix}-message")))
                .test_support()
                .aria_label(notice.message),
        );
        body = body.children(sign_in_notes(remote, prefix));
        let mut actions = div().flex().flex_wrap().gap(px(8.));
        for (index, action) in notice.actions.iter().copied().enumerate() {
            let label = action.label();
            let variant = if index == 0 {
                Variant::Default
            } else {
                Variant::Outline
            };
            // A pending sign-in keeps its own actions available.
            let disabled = busy && action != RemoteNoticeAction::CancelSignIn;
            actions = actions.child(
                button(variant, 28., disabled)
                    .id(SharedString::from(format!("{buttons}:{label}")))
                    .test_support()
                    .aria_label(label)
                    .child(label)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.remote_notice_action(action, window, cx)
                    })),
            );
        }
        body.child(actions)
    }
}

/// The session is open and needs no notice.
pub(super) fn is_live(remote: &RemoteConnectionSummary) -> bool {
    remote.connected && remote.notice().is_none()
}

/// Settings shows the token field for token servers that are not connected.
pub(super) fn needs_token(remote: Option<&RemoteConnectionSummary>) -> bool {
    remote.is_none_or(|remote| remote.auth_mode == RemoteAuthMode::Token && !is_live(remote))
}
