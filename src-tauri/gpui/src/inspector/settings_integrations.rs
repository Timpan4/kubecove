//! Remote sign-in uses only the typed host seam.
use super::remote_notice;
use super::request::InFlight;
use super::{
    Inspector, Page,
    controls::{Variant, button},
    settings_page::settings_row,
};
use crate::targets::TestSupportExt;
use crate::{backend::RemoteConnectionSummary, theme::Tokens, ui};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{Context, Div, Entity, SharedString, Task, Window, div, prelude::*, px};
use kubecove_host::remote::RemoteAuthMode;

pub(super) struct Integrations {
    _subscriptions: Vec<gpui_kit::Subscription>,
    url: Entity<InputState>,
    token: Entity<InputState>,
    pub(super) remote: Option<RemoteConnectionSummary>,
    pub(super) remote_error: Option<String>,
    /// The one remote change in flight: Connect, Disconnect, Sign out or Start a new
    /// session. The remote controls are disabled while it runs.
    pub(super) remote_call: Option<InFlight>,
    /// The browser sign-in, which runs until it finishes or is cancelled. Its
    /// notice keeps Cancel usable.
    pub(super) sign_in: Option<InFlight>,
    /// The latest status read. A newer read or a remote change replaces it.
    pub(super) status_read: Option<InFlight>,
    /// Reads the status again on each session change. Dropping it stops the watch.
    pub(super) session_watch: Option<Task<()>>,
    /// Set by a runtime change. The web resets every query then, and its Resources query reads
    /// again only when the page next mounts. The scope read that the change starts also feeds
    /// the sidebar here, so Resources reads again on its next entry instead of keeping rows a
    /// later remote drop would make stale.
    scope_reread: bool,
}
impl Integrations {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let url = cx.new(|cx| InputState::new(window, cx).placeholder("https://kube.example.com"));
        let token = cx.new(|cx| InputState::new(window, cx).masked(true));
        let subscriptions = [&url, &token]
            .into_iter()
            .map(|input| {
                cx.subscribe_in(
                    input,
                    window,
                    |_, _, _: &gpui_kit::component::input::InputEvent, _, cx| cx.notify(),
                )
            })
            .collect();
        Self {
            _subscriptions: subscriptions,
            url,
            token,
            remote: None,
            remote_error: None,
            remote_call: None,
            sign_in: None,
            status_read: None,
            session_watch: None,
            scope_reread: false,
        }
    }
}

impl Inspector {
    pub(super) fn initialize_settings_integrations(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let read = self.backend.remote_status();
        let read = self.timings.measure("remote_status", read);
        self.settings.integrations.status_read = Some(
            cx.spawn_in(window, async move |this, cx| {
                let result = read.await;
                let _ = this.update_in(cx, |this, window, cx| {
                    match result {
                        Ok(remote) => {
                            if let Some(remote) = &remote {
                                this.settings.integrations.url.update(cx, |input, cx| {
                                    input.set_value(remote.server_url.clone(), window, cx)
                                });
                            }
                            this.settings.integrations.remote = remote;
                        }
                        Err(error) => this.settings.integrations.remote_error = Some(error.message),
                    }
                    cx.notify();
                });
            })
            .into(),
        );
    }

    /// Starts a remote change. Its summary supersedes a pending status read.
    pub(super) fn start_remote_call(&mut self, task: Task<()>, cx: &mut Context<Self>) {
        let state = &mut self.settings.integrations;
        state.remote_call = Some(task.into());
        state.status_read = None;
        state.remote_error = None;
        cx.notify();
    }

    fn connect_settings_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.integrations.remote_call.is_some() || !self.can_leave(cx) {
            return;
        }
        let state = &mut self.settings.integrations;
        let url = state.url.read(cx).value().to_string();
        let token = state.token.read(cx).value().to_string();
        // An empty token selects the server by its sign-in mode (ADR 0031).
        if url.trim().is_empty() {
            return;
        }
        let read = self.backend.connect_remote(url, token);
        let read = self.timings.measure("connect_remote", read);
        // Clear the credential immediately, including failed attempts.
        state
            .token
            .update(cx, |input, cx| input.set_value("", window, cx));
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settings.integrations.remote_call = None;
                match result {
                    Ok(summary) => {
                        this.settings.integrations.remote = Some(summary);
                        this.runtime_changed(window, cx);
                    }
                    Err(error) => this.settings.integrations.remote_error = Some(error.message),
                }
                cx.notify();
            });
        });
        self.start_remote_call(task, cx);
    }
    pub(super) fn disconnect_settings_remote(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings.integrations.remote_call.is_some() || !self.can_leave(cx) {
            return;
        }
        let read = self.backend.disconnect_remote();
        let read = self.timings.measure("disconnect_remote", read);
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settings.integrations.remote_call = None;
                match result {
                    Ok(()) => {
                        this.settings.integrations.remote = None;
                        this.runtime_changed(window, cx);
                    }
                    Err(error) => this.settings.integrations.remote_error = Some(error.message),
                }
                cx.notify();
            });
        });
        self.start_remote_call(task, cx);
    }
    pub(super) fn runtime_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.feedback = Default::default();
        // The open workspace survives a runtime change, as the web's selected workspace does.
        let active = self.launcher.active;
        self.launcher = super::launcher::LauncherState::new(window, cx);
        self.rbac = super::rbac::RbacState::new(window, cx);
        self.load_preferences(cx);
        self.relink_active_workspace(active);
        self.context_task.take();
        self.observation.clear();
        self.map.read.cancel();
        self.request = std::rc::Rc::new(());
        self.close_detail(cx);
        self.live.reset_scope();
        self.visible = Default::default();
        self.row_index.invalidate();
        self.namespaces.clear();
        self.warnings.clear();
        self.scope = Default::default();
        self.settings.sources = None;
        self.settings.integrations.scope_reread = true;
        let sources = self.kubeconfig_sources_read(cx);
        self.run_kubeconfig(sources, window, cx);
        self.load_contexts(window, cx);
    }
    /// Reads the scope again when a page showing its rows opens after a runtime change.
    pub(super) fn sync_scope_reread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings.integrations.scope_reread
            || !matches!(self.page, Page::Resources | Page::ClusterOverview)
        {
            return;
        }
        self.settings.integrations.scope_reread = false;
        // Without a context the contexts read is still running, and the scope read it starts is
        // this page's read.
        if self.workspace_open() && !self.scope.context.is_empty() {
            self.load_scope(false, window, cx);
        }
    }
    pub(super) fn remote_settings_section(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.settings.integrations;
        let busy = state.remote_call.is_some();
        let mut body = div().flex().flex_col().gap(px(12.)).w(px(436.));
        if let Some(remote) = &state.remote {
            let live = remote_notice::is_live(remote);
            let status = if live {
                format!("Connected to {}.", remote.server_url)
            } else {
                format!(
                    "{} is selected but disconnected. Cluster commands will not fall back to this laptop.",
                    remote.server_url
                )
            };
            body = body.child(
                ui::sm(status.clone())
                    .id("remote-status")
                    .test_support()
                    .aria_label(status),
            );
            // The web shows the forwarding note and server details only while connected.
            if live {
                body = body
                    .child(ui::muted(ui::xs("Kubernetes requests and port forwards run through this server. Forwarded ports listen only on 127.0.0.1 on this laptop.")).id("remote-forward-explanation").test_support().aria_label("Forwarded ports listen only on 127.0.0.1 on this laptop."))
                    .child(ui::muted(ui::xs(format!(
                        "Server {}. Session grace: {} seconds.",
                        remote.server_version, remote.session_grace_seconds
                    ))).id("remote-server-session").test_support().aria_label(format!("Server {}. Session grace: {} seconds.", remote.server_version, remote.session_grace_seconds)));
            }
            if let Some(notice) = remote.notice() {
                body = body.child(self.remote_notice(
                    &notice,
                    remote,
                    remote_notice::NoticePlace::Settings,
                    cx,
                ));
            } else {
                body = body.children(remote_notice::sign_in_notes(remote, "settings"));
                let mut actions = div().flex().gap(px(8.));
                if remote.auth_mode == RemoteAuthMode::Oidc {
                    actions = actions.child(action("Sign out", busy).on_click(cx.listener(
                        |this, _, window, cx| this.sign_out_settings_remote(window, cx),
                    )));
                }
                body = body.child(actions.child(action("Disconnect", busy).on_click(
                    cx.listener(|this, _, window, cx| this.disconnect_settings_remote(window, cx)),
                )));
            }
        }
        if remote_notice::needs_token(state.remote.as_ref()) {
            body = body
                .child(field("Server URL", "remote-server-url", &state.url, busy))
                .child(field(
                    "Server token",
                    "remote-server-token",
                    &state.token,
                    busy,
                ))
                .child(
                    div().flex().child(
                        button(
                            Variant::Default,
                            28.,
                            busy || state.url.read(cx).value().trim().is_empty(),
                        )
                        .id("settings-button:Connect")
                        .test_support()
                        .aria_label("Connect")
                        .child("Connect")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.connect_settings_remote(window, cx)
                        })),
                    ),
                );
        }
        if let Some(error) = &state.remote_error {
            body = body.child(
                ui::sm(error.clone())
                    .id("remote-error")
                    .test_support()
                    .aria_label(error.clone())
                    .text_color(Tokens::destructive()),
            );
        }
        div().child(settings_row(
            "Remote KubeCove server",
            "Connect this desktop to a headless KubeCove host.".into(),
            body.into_any_element(),
        ))
    }
}

const SSO_NOTE: &str =
    "Leave the token empty to sign in to a single sign-on server through your browser.";

pub(super) fn action(label: &'static str, disabled: bool) -> super::settings_page::Observed {
    button(Variant::Outline, 32., disabled)
        .id(SharedString::from(format!("settings-button:{label}")))
        .test_support()
        .aria_label(label)
        .child(label)
}
fn field(label: &'static str, id: &'static str, input: &Entity<InputState>, disabled: bool) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(ui::medium(ui::xs(label)).line_height(px(12.)))
        .child(
            Input::new(input)
                .id(id)
                .aria_label(label)
                .disabled(disabled)
                .text_xs()
                .h_7(),
        )
        .when(label == "Server token", |field| {
            field.child(
                ui::muted(ui::xs(
                    "The token stays in native Rust memory until you disconnect or close the app.",
                ))
                .id("remote-token-note")
                .test_support()
                .aria_label(
                    "The token stays in native Rust memory until you disconnect or close the app.",
                ),
            )
            .child(
                ui::muted(ui::xs(SSO_NOTE))
                    .id("remote-token-sso-note")
                    .test_support()
                    .aria_label(SSO_NOTE),
            )
        })
}
