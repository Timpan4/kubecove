//! Session profiles are scoped by workspace, context and kubeconfig source.
use super::{
    Inspector,
    controls::{Variant, button},
    detail::Detail,
};
use crate::{backend::ArgoConnect, theme::Tokens, ui};
use gpui_kit::{
    AppContext, Context, Div, Entity, Task, TestSupportExt, Window,
    component::input::{Input, InputState},
    div,
    prelude::*,
    px,
};
use kubecove_lib::models::{ArgoConnectionProfile, ArgoConnectionStatus, ArgoServerEndpoint};
use std::rc::Rc;

#[derive(Default)]
pub(super) enum Preference {
    #[default]
    Automatic,
    Kubernetes,
    Connected(String),
}
pub(super) struct Connections {
    pub open: bool,
    pub dialog: bool,
    pub profiles: Vec<ArgoConnectionStatus>,
    pub selected: Preference,
    editing: Option<ArgoConnectionProfile>,
    scope: String,
    pub url: Entity<InputState>,
    token: Entity<InputState>,
    pub error: Option<String>,
    pub busy: bool,
    request: Rc<()>,
    task: Option<Task<()>>,
}
impl Connections {
    pub fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        Self {
            open: false,
            dialog: false,
            profiles: vec![],
            selected: Preference::Automatic,
            editing: None,
            scope: String::new(),
            url: cx
                .new(|cx| InputState::new(window, cx).placeholder("https://argocd.example.test")),
            token: cx.new(|cx| InputState::new(window, cx).masked(true)),
            error: None,
            busy: false,
            request: Rc::new(()),
            task: None,
        }
    }
}
impl Inspector {
    pub(super) fn sync_argo_scope(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scope = format!(
            "{}:{}:{:?}",
            self.argo_workspace_id(),
            self.preset_key(),
            self.argo_source_key()
        );
        if self.argo_connections.scope != scope {
            self.cancel_argo_connection(window, cx);
            self.argo_connections.scope = scope;
            self.argo_connections.error = None;
            self.argo_connections.selected = Preference::Automatic;
        }
    }
    pub(super) fn argo_workspace_id(&self) -> String {
        self.launcher
            .active
            .map_or_else(|| "native-session".into(), |id| id.to_string())
    }
    fn argo_source_key(&self) -> Option<&str> {
        self.sessions
            .load
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .and_then(|s| s.source_key.as_deref())
    }
    pub(super) fn argo_connection_id(&self) -> Option<String> {
        if matches!(self.argo_connections.selected, Preference::Kubernetes) {
            return None;
        }
        let workspace = self.argo_workspace_id();
        self.argo_connections
            .profiles
            .iter()
            .filter(|s| s.connected)
            .filter_map(|s| s.profile.as_ref())
            .find(|p| {
                p.cluster_context.as_deref() == Some(&self.scope.context)
                    && p.workspace_id.as_deref() == Some(&workspace)
                    && p.kubeconfig_source_key.as_deref() == self.argo_source_key()
                    && match &self.argo_connections.selected {
                        Preference::Connected(id) => id == &p.id,
                        Preference::Automatic => true,
                        Preference::Kubernetes => false,
                    }
            })
            .map(|p| p.id.clone())
    }
    pub(super) fn reload_argo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Detail::Ready(d) = &mut self.detail {
            d.argo = None;
        }
        self.open_argo(window, cx);
        self.load_application_workspace(cx);
    }
    pub(super) fn cancel_argo_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.argo_connections.request = Rc::new(());
        self.argo_connections.task.take();
        self.argo_connections.busy = false;
        self.argo_connections.open = false;
        self.argo_connections.dialog = false;
        self.argo_connections.editing = None;
        self.argo_connections
            .token
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }
    fn connect_argo_profile(
        &mut self,
        saved: Option<ArgoConnectionProfile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.argo_connections.busy {
            return;
        }
        let url = self.argo_connections.url.read(cx).value().trim().to_owned();
        let profile = saved.unwrap_or_else(|| ArgoConnectionProfile {
            id: format!(
                "argo:{}:{}:external:{url}",
                self.argo_workspace_id(),
                self.scope.context
            ),
            url: url.clone(),
            endpoint: ArgoServerEndpoint::ExternalHttps { url },
            cluster_context: Some(self.scope.context.clone()),
            workspace_id: Some(self.argo_workspace_id()),
            kubeconfig_source_key: self.argo_source_key().map(str::to_owned),
            transport: "connected".into(),
            remember_credential: false,
        });
        let token = self.argo_connections.token.read(cx).value().to_string();
        self.argo_connections
            .token
            .update(cx, |input, cx| input.set_value("", window, cx));
        let read = self.backend.connect_argo(ArgoConnect {
            profile,
            token: (!token.is_empty()).then_some(token),
        });
        self.argo_connections.request = Rc::new(());
        let request = self.argo_connections.request.clone();
        let scope = self.preset_key();
        self.argo_connections.busy = true;
        self.argo_connections.error = None;
        self.argo_connections.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !Rc::ptr_eq(&request, &this.argo_connections.request)
                    || this.preset_key() != scope
                {
                    return;
                }
                this.argo_connections.busy = false;
                match result {
                    Ok(status) if status.connected && status.profile.is_some() => {
                        let id = status.profile.as_ref().unwrap().id.clone();
                        this.argo_connections
                            .profiles
                            .retain(|s| s.profile.as_ref().is_none_or(|p| p.id != id));
                        this.argo_connections.selected = Preference::Connected(id);
                        this.argo_connections.profiles.push(status);
                        this.argo_connections.open = false;
                        this.argo_connections.dialog = false;
                        this.reload_argo(window, cx);
                    }
                    Ok(status) => {
                        this.argo_connections.error = Some(
                            status
                                .unavailable_reason
                                .unwrap_or_else(|| "Argo CD did not connect".into()),
                        )
                    }
                    Err(error) => this.argo_connections.error = Some(error.message),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn disconnect_argo_profile(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let read = self.backend.disconnect_argo(id.clone());
        self.argo_connections.request = Rc::new(());
        let request = self.argo_connections.request.clone();
        self.argo_connections.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !Rc::ptr_eq(&request, &this.argo_connections.request) {
                    return;
                }
                match result {
                    Ok(()) => {
                        for status in &mut this.argo_connections.profiles {
                            if status.profile.as_ref().is_some_and(|p| p.id == id) {
                                status.connected = false;
                            }
                        }
                        this.argo_connections.selected = Preference::Automatic;
                        this.reload_argo(window, cx);
                    }
                    Err(error) => this.argo_connections.error = Some(error.message),
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn argo_connection_panel(&self, cx: &mut Context<Self>) -> Div {
        if self.argo_connections.dialog {
            return div();
        }
        self.argo_connection_content(cx)
    }
    pub(super) fn argo_connection_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        self.argo_connections.dialog.then(|| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(Tokens::background().opacity(0.8))
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation()
                })
                .child(
                    div()
                        .id("argo-connection-dialog")
                        .test_support()
                        .role(gpui_kit::Role::Dialog)
                        .aria_label("Argo CD connection")
                        .w(px(480.))
                        .max_h_full()
                        .overflow_y_scroll()
                        .child(self.argo_connection_content(cx))
                        .child(
                            button(Variant::Outline, 24., false)
                                .id("argo-connection-close")
                                .test_support()
                                .child("Close connection")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_argo_connection(window, cx)
                                })),
                        ),
                )
                .into_any_element()
        })
    }
    fn argo_connection_content(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.argo_connections;
        let active = self.argo_connection_id();
        let mut panel = super::argo_panels::panel()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                button(Variant::Outline, 24., false)
                    .id("argo-connect-open")
                    .test_support()
                    .child("Connect Argo CD")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.argo_connections.editing = None;
                        this.argo_connections.open = true;
                        cx.notify();
                    })),
            );
        for status in &state.profiles {
            let Some(p) = &status.profile else {
                continue;
            };
            if p.cluster_context.as_deref() != Some(&self.scope.context)
                || p.workspace_id.as_deref() != Some(&self.argo_workspace_id())
                || p.kubeconfig_source_key.as_deref() != self.argo_source_key()
            {
                continue;
            }
            let (id, saved) = (p.id.clone(), p.clone());
            let selected_id = id.clone();
            if status.connected {
                panel = panel.child(
                    button(Variant::Outline, 24., false)
                        .id(gpui_kit::SharedString::from(format!("argo-use:{id}")))
                        .test_support()
                        .child(format!("Inspect with {}", p.url))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.argo_connections.selected =
                                Preference::Connected(selected_id.clone());
                            this.reload_argo(window, cx);
                        })),
                );
            }
            panel = panel.child(
                div().flex().gap(px(8.)).child(ui::xs(p.url.clone())).child(
                    button(Variant::Outline, 24., false)
                        .id(gpui_kit::SharedString::from(format!(
                            "argo-profile:{}",
                            p.id
                        )))
                        .test_support()
                        .child(if status.connected {
                            "Disconnect"
                        } else {
                            "Reconnect"
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if status_connected(&this.argo_connections, &id) {
                                this.disconnect_argo_profile(id.clone(), window, cx);
                            } else {
                                this.argo_connections.url.update(cx, |input, cx| {
                                    input.set_value(saved.url.clone(), window, cx)
                                });
                                this.argo_connections.editing = Some(saved.clone());
                                this.argo_connections.open = true;
                                cx.notify();
                            }
                        })),
                ),
            );
        }
        panel = panel.child(
            div()
                .flex()
                .gap(px(8.))
                .child(
                    button(Variant::Outline, 24., false)
                        .id("argo-automatic")
                        .test_support()
                        .child("Automatic")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.argo_connections.selected = Preference::Automatic;
                            this.reload_argo(window, cx);
                        })),
                )
                .child(
                    button(Variant::Outline, 24., false)
                        .id("argo-kubernetes")
                        .test_support()
                        .child("Kubernetes")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.argo_connections.selected = Preference::Kubernetes;
                            this.reload_argo(window, cx);
                        })),
                ),
        );
        panel = panel.child(ui::muted(ui::xs(if active.is_some() {
            "Connected Argo CD authorization is evaluated during operation preflight."
        } else {
            "Argo RBAC is not evaluated. Exact manifests require a connected profile."
        })));
        if state.open {
            panel = panel
                .child(ui::sm("Argo CD connection"))
                .child(
                    Input::new(&state.url)
                        .id("argo-url")
                        .aria_label("Argo CD server URL"),
                )
                .child(
                    Input::new(&state.token)
                        .id("argo-token")
                        .aria_label("Argo CD token"),
                )
                .child(
                    button(Variant::Default, 24., state.busy)
                        .id("argo-connect-submit")
                        .test_support()
                        .child("Connect")
                        .on_click(cx.listener(|this, _, window, cx| {
                            let saved = this.argo_connections.editing.clone();
                            this.connect_argo_profile(saved, window, cx)
                        })),
                )
                .child(
                    button(Variant::Outline, 24., false)
                        .id("argo-connect-cancel")
                        .test_support()
                        .child("Cancel")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.cancel_argo_connection(window, cx)
                        })),
                );
        }
        panel.when_some(state.error.clone(), |p, error| {
            p.child(
                ui::xs(error.clone())
                    .id("argo-connection-error")
                    .test_support()
                    .aria_label(error)
                    .text_color(Tokens::destructive()),
            )
        })
    }
}
fn status_connected(state: &Connections, id: &str) -> bool {
    state
        .profiles
        .iter()
        .any(|s| s.connected && s.profile.as_ref().is_some_and(|p| p.id == id))
}
