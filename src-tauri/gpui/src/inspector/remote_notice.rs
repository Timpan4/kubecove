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
};

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
    /// Reads the session state again after a remote call failed, so a moved,
    /// expired or refused session shows its notice without opening Settings.
    pub(super) fn sync_remote_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let failures = self.backend.remote_failures();
        let state = &mut self.settings.integrations;
        if failures == state.failures_seen {
            return;
        }
        state.failures_seen = failures;
        if state.remote.is_some() {
            self.refresh_remote_status(window, cx);
        }
    }

    fn refresh_remote_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let read = self.backend.remote_status();
        self.settings.integrations.status_task =
            Some(cx.spawn_in(window, async move |this, cx| {
                let result = read.await;
                let _ = this.update(cx, |this, cx| {
                    if let Ok(remote) = result {
                        this.settings.integrations.remote = remote;
                    }
                    cx.notify();
                });
            }));
    }

    pub(super) fn remote_notice_action(
        &mut self,
        action: RemoteNoticeAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            RemoteNoticeAction::SignIn => self.sign_in_remote(window, cx),
            RemoteNoticeAction::CopySignInLink => self.copy_sign_in_link(window, cx),
            RemoteNoticeAction::CancelSignIn => {
                let read = self.backend.remote_cancel_sign_in();
                self.settings.integrations.status_task =
                    Some(cx.spawn_in(window, async move |this, cx| {
                        let result = read.await;
                        let _ = this.update_in(cx, |this, window, cx| match result {
                            Ok(()) => this.refresh_remote_status(window, cx),
                            Err(error) => {
                                this.settings.integrations.remote_error = Some(error.message);
                                cx.notify();
                            }
                        });
                    }));
            }
            RemoteNoticeAction::StartNewSession => {
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
        state.sign_in_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(summary) => this.apply_remote_summary(summary, window, cx),
                    Err(error) => {
                        this.settings.integrations.remote_error = Some(error.message);
                        this.refresh_remote_status(window, cx);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn copy_sign_in_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let read = self.backend.remote_sign_in_link();
        self.settings.integrations.status_task =
            Some(cx.spawn_in(window, async move |this, cx| {
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
            }));
    }

    pub(super) fn sign_out_settings_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.integrations.remote_busy || !self.can_leave(cx) {
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
        let state = &mut self.settings.integrations;
        state.remote_busy = true;
        state.remote_error = None;
        state.remote_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settings.integrations.remote_busy = false;
                match result {
                    Ok(summary) => this.apply_remote_summary(Some(summary), window, cx),
                    Err(error) => this.settings.integrations.remote_error = Some(error.message),
                }
                cx.notify();
            });
        }));
        cx.notify();
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
        let memory_only = remote.sign_in_memory_only;
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
                        .child(self.remote_notice(&notice, memory_only, NoticePlace::Content, cx)),
                )
                .into_any_element(),
        )
    }

    pub(super) fn remote_notice(
        &self,
        notice: &RemoteNotice,
        memory_only: bool,
        place: NoticePlace,
        cx: &mut Context<Self>,
    ) -> Div {
        let (prefix, buttons) = (place.text(), place.button());
        let busy = self.settings.integrations.remote_busy;
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
        if memory_only {
            body = body.child(
                ui::muted(ui::xs(SIGN_IN_MEMORY_ONLY))
                    .id(SharedString::from(format!("{prefix}-memory-only")))
                    .test_support()
                    .aria_label(SIGN_IN_MEMORY_ONLY),
            );
        }
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
