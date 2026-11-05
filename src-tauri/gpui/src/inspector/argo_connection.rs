//! Workspace-scoped connection state. Credentials live only in masked inputs until submission.
use super::{Inspector, detail::Detail};
use crate::backend::ArgoConnect;
use gpui_kit::{
    AppContext, Context, Entity, Task, Window,
    component::input::{InputState, TextareaState},
};
use kubecove_lib::models::*;
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
    pub editing: Option<ArgoConnectionProfile>,
    pub scope: String,
    pub url: Entity<InputState>,
    pub token: Entity<InputState>,
    pub username: Entity<InputState>,
    pub password: Entity<InputState>,
    pub ca: Entity<TextareaState>,
    pub root_path: Entity<InputState>,
    pub tls_name: Entity<InputState>,
    pub local: bool,
    pub insecure: bool,
    pub remember: bool,
    pub servers: Vec<ArgoServerCapability>,
    pub endpoint: Option<ArgoServerEndpoint>,
    pub preview: Option<ArgoTunnelTargetPreview>,
    pub preview_endpoint: Option<ArgoServerEndpoint>,
    pub error: Option<String>,
    pub busy: bool,
    pub request: Rc<()>,
    pub task: Option<Task<()>>,
    pub discovery: Option<Task<()>>,
    pub discovery_error: Option<String>,
    pub forgetting: Option<Task<()>>,
    pub reconnecting: bool,
    pub pending_reconnect: std::collections::VecDeque<ArgoConnectionProfile>,
    pub picker: Entity<
        gpui_kit::component::select::SelectState<
            gpui_kit::component::select::SearchableVec<gpui_kit::SharedString>,
        >,
    >,
    pub _subscription: gpui_kit::Subscription,
}
impl Connections {
    pub fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectState};
        let picker = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(vec![
                    gpui_kit::SharedString::from("Automatic"),
                    "Kubernetes".into(),
                ]),
                None,
                window,
                cx,
            )
        });
        let subscription = cx.subscribe_in(&picker, window, |this, _, event, window, cx| {
            let SelectEvent::Confirm(Some(value)) = event else {
                return;
            };
            let preference = match value.as_ref() {
                "Automatic" => Preference::Automatic,
                "Kubernetes" => Preference::Kubernetes,
                label => {
                    let Some(profile) = this
                        .argo_connections
                        .profiles
                        .iter()
                        .filter_map(|s| s.profile.as_ref())
                        .find(|p| {
                            this.argo_profile_matches(p)
                                && super::argo_profiles::endpoint_label(&p.endpoint) == label
                        })
                    else {
                        return;
                    };
                    Preference::Connected(profile.id.clone())
                }
            };
            this.argo_connections.selected = preference;
            this.persist_argo_preferences(cx);
            this.reload_argo(window, cx);
        });
        Self {
            picker,
            _subscription: subscription,
            open: false,
            dialog: false,
            profiles: vec![],
            selected: Preference::Automatic,
            editing: None,
            scope: String::new(),
            url: cx
                .new(|cx| InputState::new(window, cx).placeholder("https://argocd.example.test")),
            token: cx.new(|cx| InputState::new(window, cx).masked(true)),
            username: cx.new(|cx| InputState::new(window, cx)),
            password: cx.new(|cx| InputState::new(window, cx).masked(true)),
            ca: cx.new(|cx| TextareaState::new(window, cx).rows(3)),
            root_path: cx.new(|cx| InputState::new(window, cx)),
            tls_name: cx.new(|cx| InputState::new(window, cx)),
            local: false,
            insecure: false,
            remember: false,
            servers: vec![],
            endpoint: None,
            preview: None,
            preview_endpoint: None,
            error: None,
            busy: false,
            request: Rc::new(()),
            task: None,
            discovery: None,
            discovery_error: None,
            forgetting: None,
            reconnecting: false,
            pending_reconnect: Default::default(),
        }
    }
}
impl Inspector {
    pub(super) fn reload_argo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Detail::Ready(d) = &mut self.detail {
            d.argo = None;
        }
        self.open_argo(window, cx);
        self.load_application_workspace(cx);
    }
    pub(super) fn clear_argo_secrets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.argo_connections
            .ca
            .update(cx, |input, cx| input.set_value("", window, cx));
        for field in [
            &self.argo_connections.token,
            &self.argo_connections.username,
            &self.argo_connections.password,
        ] {
            field.update(cx, |input, cx| input.set_value("", window, cx));
        }
    }
    pub(super) fn cancel_argo_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.argo_connections.request = Rc::new(());
        self.argo_connections.task.take();
        self.argo_connections.discovery.take();
        self.argo_connections.busy = self
            .argo_connections
            .forgetting
            .as_ref()
            .is_some_and(|t| !t.is_ready());
        self.argo_connections.open = false;
        self.argo_connections.dialog = false;
        self.argo_connections.editing = None;
        self.argo_connections.reconnecting = false;
        self.argo_connections.preview = None;
        self.argo_connections.preview_endpoint = None;
        self.argo_connections.insecure = false;
        self.clear_argo_secrets(window, cx);
        cx.notify();
    }
    pub(super) fn connect_argo_profile(
        &mut self,
        confirmed_target: Option<ArgoConfirmedTarget>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.argo_connections.busy {
            return;
        }
        let state = &self.argo_connections;
        let remembered = state
            .editing
            .as_ref()
            .is_some_and(|p| p.remember_credential);
        let credential = if state.local {
            !state.username.read(cx).value().is_empty()
                && !state.password.read(cx).value().is_empty()
        } else {
            !state.token.read(cx).value().is_empty()
        };
        if !credential && !remembered {
            self.argo_connections.error =
                Some("Enter a token or local username and password before connecting.".into());
            cx.notify();
            return;
        }
        let mut endpoint = if confirmed_target.is_some() && !state.reconnecting {
            state.preview_endpoint.clone()
        } else {
            None
        }
        .or_else(|| {
            state
                .editing
                .as_ref()
                .map(|p| p.endpoint.clone())
                .or_else(|| state.endpoint.clone())
        })
        .unwrap_or_else(|| ArgoServerEndpoint::ExternalHttps {
            url: state.url.read(cx).value().trim().to_owned(),
        });
        if confirmed_target.is_none()
            && state.editing.is_none()
            && let ArgoServerEndpoint::ServiceTunnel {
                root_path,
                tls_server_name,
                ..
            } = &mut endpoint
        {
            let root = state.root_path.read(cx).value().trim().to_owned();
            let name = state.tls_name.read(cx).value().trim().to_owned();
            if !root.is_empty() {
                *root_path = Some(root);
            }
            if !name.is_empty() {
                *tls_server_name = Some(name);
            }
        }
        if matches!(endpoint, ArgoServerEndpoint::ServiceTunnel { .. })
            && confirmed_target.is_none()
        {
            self.preview_argo_target(endpoint, window, cx);
            return;
        }
        let url = super::argo_profiles::endpoint_url(&endpoint);
        let external = matches!(endpoint, ArgoServerEndpoint::ExternalHttps { .. });
        let profile = ArgoConnectionProfile {
            id: state.editing.as_ref().map_or_else(
                || {
                    format!(
                        "argo:{}:{}:{}:{}",
                        self.argo_workspace_id(),
                        self.scope.context,
                        self.argo_source_key().unwrap_or("global"),
                        super::argo_profiles::endpoint_identity(&endpoint)
                    )
                },
                |p| p.id.clone(),
            ),
            url,
            endpoint,
            cluster_context: Some(self.scope.context.clone()),
            workspace_id: Some(self.argo_workspace_id()),
            kubeconfig_source_key: self.argo_source_key().map(str::to_owned),
            transport: "connected".into(),
            remember_credential: state.remember
                || state
                    .editing
                    .as_ref()
                    .is_some_and(|p| p.remember_credential),
        };
        let value = |field: &Entity<InputState>| {
            let s = field.read(cx).value().to_string();
            (!s.is_empty()).then_some(s)
        };
        let read = self.backend.connect_argo(ArgoConnect {
            profile,
            token: if state.local {
                None
            } else {
                value(&state.token)
            },
            username: if state.local {
                value(&state.username)
            } else {
                None
            },
            password: if state.local {
                value(&state.password)
            } else {
                None
            },
            insecure_tls: external && state.insecure,
            custom_ca_pem: if external {
                let ca = state.ca.read(cx).value().to_string();
                (!ca.is_empty()).then(|| ca.into_bytes())
            } else {
                None
            },
            confirmed_target,
        });
        self.clear_argo_secrets(window, cx);
        self.argo_connections.preview = None;
        self.argo_connections.request = Rc::new(());
        let request = self.argo_connections.request.clone();
        self.argo_connections.busy = true;
        self.argo_connections.error = None;
        self.argo_connections.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !Rc::ptr_eq(&request, &this.argo_connections.request) {
                    return;
                }
                this.argo_connections.busy = false;
                let automatic = this.argo_connections.reconnecting;
                match result {
                    Ok(status) if status.connected && status.profile.is_some() => {
                        let id = status.profile.as_ref().unwrap().id.clone();
                        if let Some(existing) = this
                            .argo_connections
                            .profiles
                            .iter_mut()
                            .find(|s| s.profile.as_ref().is_some_and(|p| p.id == id))
                        {
                            *existing = status;
                        } else {
                            this.argo_connections.profiles.push(status);
                        }
                        if !this.argo_connections.reconnecting {
                            this.argo_connections.selected = Preference::Connected(id);
                        }
                        this.argo_connections.reconnecting = false;
                        this.persist_argo_preferences(cx);
                        this.sync_argo_picker(window, cx);
                        this.argo_connections.open = false;
                        this.argo_connections.dialog = false;
                        this.argo_connections.insecure = false;
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
                if automatic {
                    this.reconnect_next_argo_profile(window, cx);
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
        if self.argo_connections.busy {
            return;
        }
        self.argo_connections.busy = true;
        let read = self.backend.disconnect_argo(id.clone());
        self.argo_connections.request = Rc::new(());
        let request = self.argo_connections.request.clone();
        self.argo_connections.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !Rc::ptr_eq(&request, &this.argo_connections.request) {
                    return;
                }
                this.argo_connections.busy = false;
                match result {
                    Ok(()) => {
                        for status in &mut this.argo_connections.profiles {
                            if status.profile.as_ref().is_some_and(|p| p.id == id) {
                                status.connected = false;
                            }
                        }
                        this.argo_connections.selected = Preference::Automatic;
                        this.persist_argo_preferences(cx);
                        this.sync_argo_picker(window, cx);
                        this.reload_argo(window, cx);
                    }
                    Err(error) => this.argo_connections.error = Some(error.message),
                }
                cx.notify();
            });
        }));
    }
}
