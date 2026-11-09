//! Workspace-scoped connection state. Credentials live only in masked inputs until submission.
use super::{Inspector, detail::Detail, request::InFlight};
use crate::backend::ArgoConnect;
use gpui_kit::{
    AppContext, Context, Entity, FocusHandle, Window,
    component::input::{InputState, TextareaState},
};
use kubecove_lib::models::*;

#[derive(Default)]
pub(super) enum Preference {
    #[default]
    Automatic,
    Kubernetes,
    Connected(String),
}
/// The dialog's connect flow, as the web's `ConnectStep`. Closing the dialog cancels a
/// `Busy` read. Disconnect and Forget run in `Connections::profile_action` and automatic
/// reconnects in `Connections::reconnect`, so closing the dialog leaves them running.
pub(super) enum ConnectStep {
    Idle,
    /// A tunnel preview or a connect read. Replacing the step cancels it.
    Busy { _read: InFlight },
    /// A previewed tunnel target that waits for the user's confirmation.
    ConfirmTarget {
        endpoint: ArgoServerEndpoint,
        preview: ArgoTunnelTargetPreview,
    },
    /// `reconfirm` is the remembered tunnel profile whose reconnect failed. Only it
    /// offers a fresh preview of its saved target, as on the web.
    Failed {
        message: String,
        reconfirm: Option<ArgoConnectionProfile>,
    },
}
impl ConnectStep {
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
            reconfirm: None,
        }
    }
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed { message, .. } => Some(message),
            _ => None,
        }
    }
    /// Drops a previewed target after the endpoint choice changes.
    pub fn clear_preview(&mut self) {
        if matches!(self, Self::ConfirmTarget { .. }) {
            *self = Self::Idle;
        }
    }
}
/// Remembered profiles that reconnect one at a time after the workspace scope changes.
/// Only a scope change replaces it.
#[derive(Default)]
pub(super) struct Reconnect {
    pub queue: std::collections::VecDeque<ArgoConnectionProfile>,
    pub current: Option<InFlight>,
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
    pub step: ConnectStep,
    /// A running Disconnect or Forget.
    pub profile_action: Option<InFlight>,
    pub reconnect: Reconnect,
    pub discovery: Option<gpui_kit::Task<()>>,
    pub discovery_error: Option<String>,
    pub picker: Entity<
        gpui_kit::component::select::SelectState<
            gpui_kit::component::select::SearchableVec<gpui_kit::SharedString>,
        >,
    >,
    /// Focus container of the connection dialog. Its focus trap keeps Tab and
    /// Shift-Tab inside the dialog, as the web's dialog does.
    pub dialog_focus: FocusHandle,
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
            dialog_focus: cx.focus_handle(),
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
            step: ConnectStep::Idle,
            profile_action: None,
            reconnect: Reconnect::default(),
            discovery: None,
            discovery_error: None,
        }
    }
    /// The dialog blocks new connects and profile actions while one runs.
    pub fn busy(&self) -> bool {
        matches!(self.step, ConnectStep::Busy { .. }) || self.profile_action.is_some()
    }
}
/// A connect result as the status to record, or the message to show.
pub(super) fn connected(
    result: Result<ArgoConnectionStatus, AppError>,
) -> Result<ArgoConnectionStatus, String> {
    match result {
        Ok(status) if status.connected && status.profile.is_some() => Ok(status),
        Ok(status) => Err(status
            .unavailable_reason
            .unwrap_or_else(|| "Argo CD did not connect".into())),
        Err(error) => Err(error.message),
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
    /// Closes the dialog and cancels its preview or connect. A failure stays visible.
    pub(super) fn cancel_argo_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.argo_connections;
        if !matches!(state.step, ConnectStep::Failed { .. }) {
            state.step = ConnectStep::Idle;
        }
        state.discovery.take();
        state.open = false;
        state.dialog = false;
        state.editing = None;
        self.clear_argo_secrets(window, cx);
        cx.notify();
    }
    /// Records a connected profile and saves it, as the web's connection status cache does.
    pub(super) fn record_argo_status(
        &mut self,
        status: ArgoConnectionStatus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = status.profile.as_ref().map(|p| p.id.clone());
        if let Some(existing) = self
            .argo_connections
            .profiles
            .iter_mut()
            .find(|s| s.profile.as_ref().map(|p| &p.id) == id.as_ref())
        {
            *existing = status;
        } else {
            self.argo_connections.profiles.push(status);
        }
        // The web changes the inspection preference only when the user picks it.
        self.persist_argo_preferences(cx);
        self.sync_argo_picker(window, cx);
    }
    pub(super) fn connect_argo_profile(
        &mut self,
        confirmed_target: Option<ArgoConfirmedTarget>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.argo_connections.busy() {
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
            self.argo_connections.step = ConnectStep::failed(
                "Enter a token or local username and password before connecting.",
            );
            cx.notify();
            return;
        }
        let previewed = match (&confirmed_target, &state.step) {
            (Some(_), ConnectStep::ConfirmTarget { endpoint, .. }) => Some(endpoint.clone()),
            _ => None,
        };
        let mut endpoint = previewed
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
        let reconfirm = state.editing.clone().filter(|p| {
            p.remember_credential && matches!(p.endpoint, ArgoServerEndpoint::ServiceTunnel { .. })
        });
        self.clear_argo_secrets(window, cx);
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match connected(result) {
                    Ok(status) => {
                        this.argo_connections.step = ConnectStep::Idle;
                        this.record_argo_status(status, window, cx);
                        this.argo_connections.open = false;
                        this.argo_connections.dialog = false;
                        this.reload_argo(window, cx);
                    }
                    Err(message) => {
                        this.argo_connections.step = ConnectStep::Failed { message, reconfirm }
                    }
                }
                cx.notify();
            });
        });
        self.argo_connections.step = ConnectStep::Busy { _read: task.into() };
        cx.notify();
    }
    pub(super) fn disconnect_argo_profile(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.argo_connections.busy() {
            return;
        }
        let read = self.backend.disconnect_argo(id.clone());
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.argo_connections.profile_action = None;
                match result {
                    Ok(()) => {
                        for status in &mut this.argo_connections.profiles {
                            if status.profile.as_ref().is_some_and(|p| p.id == id) {
                                status.connected = false;
                            }
                        }
                        // The saved preference stays, as on the web. A preferred profile that is
                        // now disconnected is unavailable, and the snapshot stays until it reconnects.
                        this.sync_argo_picker(window, cx);
                        if this.argo_connection_unavailable().is_none() {
                            this.reload_argo(window, cx);
                        }
                    }
                    Err(error) => this.argo_connections.step = ConnectStep::failed(error.message),
                }
                cx.notify();
            });
        });
        self.argo_connections.step = ConnectStep::Idle;
        self.argo_connections.profile_action = Some(task.into());
        cx.notify();
    }
}
