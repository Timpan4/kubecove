//! The desktop MCP host remains local. Tokens live only in the explicit copy task.
use super::{
    Inspector,
    controls::{Variant, button},
    request::InFlight,
    settings_page::{Observed, settings_row, switch},
};
use crate::targets::TestSupportExt;
use crate::{
    backend::Read,
    settings::{self, Category},
    theme::Tokens,
    ui,
};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    ClipboardItem, Context, Div, Entity, SharedString, Subscription, Window, div, prelude::*, px,
};
use kubecove_lib::models::McpStatus;

pub(super) struct McpState {
    status: Option<McpStatus>,
    port: Entity<InputState>,
    /// The status read, settings change, token rotation or token copy in
    /// flight. The controls are disabled while it runs.
    call: Option<InFlight>,
    error: Option<String>,
    announcement: String,
    _subscription: Subscription,
}
impl McpState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let port = cx.new(|cx| InputState::new(window, cx));
        let subscription =
            cx.subscribe_in(&port, window, |this, _, event, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.change_mcp(true, window, cx),
                _ => {}
            });
        Self {
            status: None,
            port,
            call: None,
            error: None,
            announcement: String::new(),
            _subscription: subscription,
        }
    }
    fn port(&self, cx: &Context<Inspector>) -> Option<u16> {
        let text = self.port.read(cx).value();
        let text = text.trim();
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        text.parse().ok().filter(|port| *port != 0)
    }
    /// The status the controls act on: known, with no call in flight.
    fn ready(&self) -> Option<&McpStatus> {
        self.status.as_ref().filter(|_| self.call.is_none())
    }
}

impl Inspector {
    pub(super) fn initialize_mcp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.mcp.status = None;
        self.run_mcp(self.backend.mcp_status(), true, window, cx);
    }
    fn run_mcp(
        &mut self,
        read: Read<McpStatus>,
        sync_port: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let call = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let state = &mut this.settings.mcp;
                state.call = None;
                match result {
                    Ok(status) => {
                        if sync_port {
                            state.port.update(cx, |input, cx| {
                                input.set_value(status.port.to_string(), window, cx)
                            });
                        }
                        state.status = Some(status);
                    }
                    Err(error) => state.error = Some(error.message),
                }
                cx.notify();
            });
        });
        let state = &mut self.settings.mcp;
        state.error = None;
        state.call = Some(call.into());
        cx.notify();
    }
    fn change_mcp(&mut self, save_port: bool, window: &mut Window, cx: &mut Context<Self>) {
        let state = &self.settings.mcp;
        let Some(status) = state.ready() else {
            return;
        };
        let (enabled, port) = if save_port {
            let Some(port) = state.port(cx).filter(|port| *port != status.port) else {
                return;
            };
            (status.enabled, port)
        } else {
            (!status.enabled, status.port)
        };
        self.run_mcp(
            self.backend.set_mcp_settings(enabled, port),
            save_port,
            window,
            cx,
        );
    }
    fn rotate_mcp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.mcp.ready().is_some() {
            self.run_mcp(self.backend.rotate_mcp_token(), false, window, cx);
        }
    }
    fn copy_mcp(&mut self, command: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(status) = self.settings.mcp.ready() else {
            return;
        };
        // Capture the URL before the asynchronous credential read, matching Svelte.
        let url = status.url.clone();
        if command && url.is_none() {
            return;
        }
        let read = self.backend.reveal_mcp_token();
        let call = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.settings.mcp;
                state.call = None;
                match result {
                    Ok(token) => {
                        let value = match url.filter(|_| command) {
                            Some(url) => format!("claude mcp add --transport http kubecove {url} --header \"Authorization: Bearer {token}\""),
                            None => token,
                        };
                        cx.write_to_clipboard(ClipboardItem::new_string(value));
                        state.announcement = if command { "Copied Claude Code command." } else { "Copied token." }.into();
                    }
                    Err(error) => state.error = Some(error.message),
                }
                cx.notify();
            });
        });
        let state = &mut self.settings.mcp;
        state.announcement.clear();
        state.call = Some(call.into());
        cx.notify();
    }
    pub(super) fn mcp_settings_section(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.settings.mcp;
        let ready = state.ready().is_some();
        let enabled = state.status.as_ref().is_some_and(|status| status.enabled);
        let running = state.status.as_ref().is_some_and(|status| status.running);
        let rows: Vec<_> = settings::rows(Category::Agents, false).collect();
        let description = state
            .status
            .as_ref()
            .filter(|status| status.running)
            .and_then(|status| {
                status.url.as_ref().map(|url| {
                    format!(
                        "Listening at {url} with {} read-only tools.",
                        status.tools.len()
                    )
                })
            })
            .unwrap_or_else(|| rows[0].description.into());
        let mut server = div().flex().items_center().gap(px(8.));
        if enabled {
            server = server.child(
                ui::outline_badge(if running { "Running" } else { "Stopped" })
                    .when(!running, |badge| badge.text_color(Tokens::destructive())),
            );
        }
        let toggle = switch("MCP server", enabled)
            .id("mcp-control:MCP server")
            .aria_description(if ready { "enabled" } else { "disabled" })
            .when(!ready, |toggle| toggle.opacity(0.5))
            .on_click(cx.listener(|this, _, window, cx| this.change_mcp(false, window, cx)));
        let port = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                Input::new(&state.port)
                    .id("mcp-port")
                    .aria_label("MCP port")
                    .disabled(!ready)
                    .text_xs()
                    .h_7()
                    .w(px(96.)),
            )
            .child(
                action(
                    "Save",
                    !ready
                        || state.port(cx).is_none_or(|port| {
                            state
                                .status
                                .as_ref()
                                .is_none_or(|status| port == status.port)
                        }),
                )
                .on_click(cx.listener(|this, _, window, cx| this.change_mcp(true, window, cx))),
            );
        let access = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(
                action(
                    "Copy Claude Code command",
                    !ready
                        || state
                            .status
                            .as_ref()
                            .is_none_or(|status| status.url.is_none()),
                )
                .on_click(cx.listener(|this, _, window, cx| this.copy_mcp(true, window, cx))),
            )
            .child(
                action("Copy token", !ready)
                    .on_click(cx.listener(|this, _, window, cx| this.copy_mcp(false, window, cx))),
            )
            .child(
                action("Rotate token", !ready)
                    .on_click(cx.listener(|this, _, window, cx| this.rotate_mcp(window, cx))),
            );
        let mut section = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                settings_row(
                    rows[0].title,
                    description.clone(),
                    server.child(toggle).into_any_element(),
                )
                .aria_label(description),
            )
            .child(settings_row(
                rows[1].title,
                rows[1].description.into(),
                port.into_any_element(),
            ))
            .child(settings_row(
                rows[2].title,
                rows[2].description.into(),
                access.into_any_element(),
            ));
        for error in [
            state
                .status
                .as_ref()
                .and_then(|status| status.error.as_ref()),
            state.error.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            section = section.child(
                ui::sm(error.clone())
                    .id("mcp-error")
                    .test_support()
                    .aria_label(error.clone())
                    .text_color(Tokens::destructive()),
            );
        }
        section.child(
            ui::muted(ui::sm(state.announcement.clone()))
                .id("mcp-announcement")
                .test_support()
                .aria_label(state.announcement.clone()),
        )
    }
}

fn action(label: &'static str, disabled: bool) -> Observed {
    button(Variant::Outline, 24., disabled)
        .id(SharedString::from(format!("mcp-control:{label}")))
        .test_support()
        .aria_label(label)
        .aria_description(if disabled { "disabled" } else { "enabled" })
        .child(label)
}
