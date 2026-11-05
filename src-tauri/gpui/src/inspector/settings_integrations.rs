//! Remote sign-in and update status use only the typed host seam.
use super::{
    Inspector,
    controls::{Variant, button},
    settings_page::settings_row,
};
use crate::{
    backend::{INSTALL_UNAVAILABLE, RemoteConnectionSummary, UpdateStatus},
    theme::Tokens,
    ui,
};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{
    Context, Div, Entity, SharedString, Task, TestSupportExt, Window, div, prelude::*, px,
};

pub(super) struct Integrations {
    _subscriptions: Vec<gpui_kit::Subscription>,
    url: Entity<InputState>,
    token: Entity<InputState>,
    remote: Option<RemoteConnectionSummary>,
    remote_error: Option<String>,
    remote_busy: bool,
    remote_task: Option<Task<()>>,
    update: UpdateStatus,
    checked_at: Option<String>,
    update_error: Option<String>,
    update_busy: bool,
    update_task: Option<Task<()>>,
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
            remote_busy: false,
            remote_task: None,
            update: UpdateStatus::Idle,
            checked_at: None,
            update_error: None,
            update_busy: false,
            update_task: None,
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
        self.settings.integrations.remote_task =
            Some(cx.spawn_in(window, async move |this, cx| {
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
            }));
    }

    fn connect_settings_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.integrations.remote_busy || !self.can_leave(cx) {
            return;
        }
        let state = &mut self.settings.integrations;
        let url = state.url.read(cx).value().to_string();
        let token = state.token.read(cx).value().to_string();
        if url.trim().is_empty() || token.trim().is_empty() {
            return;
        }
        let read = self.backend.connect_remote(url, token);
        // Clear the credential immediately, including failed attempts.
        state
            .token
            .update(cx, |input, cx| input.set_value("", window, cx));
        state.remote_busy = true;
        state.remote_error = None;
        state.remote_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settings.integrations.remote_busy = false;
                match result {
                    Ok(summary) => {
                        this.settings.integrations.remote = Some(summary);
                        this.runtime_changed(window, cx);
                    }
                    Err(error) => this.settings.integrations.remote_error = Some(error.message),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn disconnect_settings_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.integrations.remote_busy || !self.can_leave(cx) {
            return;
        }
        let read = self.backend.disconnect_remote();
        let state = &mut self.settings.integrations;
        state.remote_busy = true;
        state.remote_error = None;
        state.remote_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.settings.integrations.remote_busy = false;
                match result {
                    Ok(()) => {
                        this.settings.integrations.remote = None;
                        this.runtime_changed(window, cx);
                    }
                    Err(error) => this.settings.integrations.remote_error = Some(error.message),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn runtime_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.launcher = super::launcher::LauncherState::new(window, cx);
        self.rbac = super::rbac_page::RbacState::new(window, cx);
        self.load_preferences(cx);
        self.context_task.take();
        self.scope_task.take();
        self.topology_task.take();
        self.request = std::rc::Rc::new(());
        self.close_detail(cx);
        self.live.reset_scope();
        self.rows.clear();
        self.visible.clear();
        self.namespaces.clear();
        self.warnings.clear();
        self.scope = Default::default();
        self.settings.sources = None;
        let sources = self.backend.kubeconfig_sources();
        self.run_kubeconfig(sources, window, cx);
        self.load_contexts(window, cx);
    }
    pub(super) fn remote_settings_section(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.settings.integrations;
        let mut body = div().flex().flex_col().gap(px(12.)).w(px(436.));
        if let Some(remote) = &state.remote {
            let status = if remote.connected {
                format!("Connected to {}.", remote.server_url)
            } else {
                format!(
                    "{} is selected but disconnected. Cluster commands will not fall back to this laptop.",
                    remote.server_url
                )
            };
            body = body
                .child(
                    ui::sm(status.clone())
                        .id("remote-status")
                        .test_support()
                        .aria_label(status),
                )
                .child(ui::muted(ui::xs(format!(
                    "Server {}. Session grace: {} seconds.",
                    remote.server_version, remote.session_grace_seconds
                ))))
                .child(action("Disconnect", state.remote_busy).on_click(
                    cx.listener(|this, _, window, cx| this.disconnect_settings_remote(window, cx)),
                ));
        }
        if !state.remote.as_ref().is_some_and(|remote| remote.connected) {
            body = body
                .child(field(
                    "Server URL",
                    "remote-server-url",
                    &state.url,
                    state.remote_busy,
                ))
                .child(field(
                    "Server token",
                    "remote-server-token",
                    &state.token,
                    state.remote_busy,
                ))
                .child(
                    div().flex().child(
                        button(
                            Variant::Default,
                            28.,
                            state.remote_busy
                                || state.url.read(cx).value().trim().is_empty()
                                || state.token.read(cx).value().trim().is_empty(),
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
    fn check_settings_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.integrations.update_busy || !self.backend.updates_enabled() {
            return;
        }
        let read = self.backend.check_updates();
        let state = &mut self.settings.integrations;
        state.update_busy = true;
        state.update_error = None;
        state.update_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.settings.integrations;
                state.update_busy = false;
                state.checked_at = Some(
                    chrono::Local::now()
                        .format("%b %-d, %Y, %-H:%M")
                        .to_string(),
                );
                this.persisted.updates.last_checked_at = state.checked_at.clone();
                match result {
                    Ok(status) => state.update = status,
                    Err(error) => {
                        state.update = UpdateStatus::Idle;
                        state.update_error = Some(error.message);
                    }
                }
                this.save_preferences(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn updates_settings_section(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.settings.integrations;
        let enabled = self.backend.updates_enabled();
        let description = if enabled {
            format!(
                "Current version {}. Last checked: {}.",
                env!("CARGO_PKG_VERSION"),
                self.persisted
                    .updates
                    .last_checked_at
                    .as_deref()
                    .unwrap_or("Not checked yet")
            )
        } else {
            format!(
                "Current version {}. Development build; update checks disabled.",
                env!("CARGO_PKG_VERSION")
            )
        };
        let mut control = div().flex().items_center().gap(px(8.));
        if enabled {
            if let UpdateStatus::Available(version) = &state.update {
                control = control.child(ui::outline_badge(format!("Update {version}")));
            }
            control = control.child(action("Check", state.update_busy).on_click(
                cx.listener(|this, _, window, cx| this.check_settings_updates(window, cx)),
            ));
        } else {
            control = control.child(ui::outline_badge("Dev build"));
        }
        let mut section = div().flex().flex_col().gap(px(16.)).child(
            settings_row("KubeCove version", description, control.into_any_element())
                .aria_label("KubeCove version"),
        );
        if enabled {
            if let UpdateStatus::Available(version) = &state.update {
                section = section
                    .child(
                        settings_row(
                            "Update available",
                            format!("KubeCove {version} can be downloaded and installed now."),
                            action("Install", true)
                                .aria_description(INSTALL_UNAVAILABLE)
                                .into_any_element(),
                        )
                        .aria_label("Update available"),
                    )
                    .child(ui::muted(ui::xs(INSTALL_UNAVAILABLE)));
            }
            if let Some(error) = &state.update_error {
                section = section.child(
                    settings_row(
                        "Update check failed",
                        error.clone(),
                        action("Retry", false)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.check_settings_updates(window, cx)
                            }))
                            .into_any_element(),
                    )
                    .aria_label(error.clone()),
                );
            }
        }
        section
    }
}

fn action(label: &'static str, disabled: bool) -> super::settings_page::Observed {
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
            field.child(ui::muted(ui::xs(
                "The token stays in native Rust memory until you disconnect or close the app.",
            )))
        })
}
