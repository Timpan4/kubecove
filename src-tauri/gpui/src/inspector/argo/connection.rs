//! Workspace-scoped connection state. Credentials live only in masked inputs until submission.
use super::super::{Inspector, detail::Detail, request::InFlight};
use super::connection_view::same_server;
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
    Busy {
        _read: InFlight,
    },
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
/// The surface whose dialog is open. The web mounts one `ArgoConnectionDialog` on the
/// GitOps page and one in each Application detail, and each keeps its own choices.
#[derive(Clone, Copy, Default)]
pub(in crate::inspector) enum DialogHost {
    #[default]
    Detail,
    Page,
}
/// The choices one web dialog keeps between opens until it unmounts.
#[derive(Default)]
pub(super) struct DialogChoices {
    pub local: bool,
    pub insecure: bool,
    pub remember: bool,
    /// Whether the user picked the server source. Until then the first discovered Service is the default.
    pub source_chosen: bool,
    /// The chosen discovered Service with its protocol edits. `None` is External URL.
    pub endpoint: Option<ArgoServerEndpoint>,
}
/// The text one web dialog keeps until it unmounts: its server URL, its Advanced fields and
/// its typed credentials.
#[derive(Clone)]
pub(super) struct DialogFields {
    pub url: Entity<InputState>,
    pub root_path: Entity<InputState>,
    pub tls_name: Entity<InputState>,
    pub ca: Entity<TextareaState>,
    pub token: Entity<InputState>,
    pub username: Entity<InputState>,
    pub password: Entity<InputState>,
}
impl DialogFields {
    fn new(window: &mut Window, cx: &mut gpui_kit::App) -> Self {
        Self {
            url: cx
                .new(|cx| InputState::new(window, cx).placeholder("https://argocd.example.test")),
            root_path: cx.new(|cx| InputState::new(window, cx)),
            tls_name: cx.new(|cx| InputState::new(window, cx)),
            ca: cx.new(|cx| TextareaState::new(window, cx).rows(3)),
            token: cx.new(|cx| InputState::new(window, cx).masked(true)),
            username: cx.new(|cx| InputState::new(window, cx)),
            password: cx.new(|cx| InputState::new(window, cx).masked(true)),
        }
    }
    /// Empties the typed credentials and the CA, as the web does after a submit.
    pub fn clear_secrets(&self, window: &mut Window, cx: &mut gpui_kit::App) {
        self.ca
            .update(cx, |input, cx| input.set_value("", window, cx));
        for field in [&self.token, &self.username, &self.password] {
            field.update(cx, |input, cx| input.set_value("", window, cx));
        }
    }
    /// Empties every field, as the web loses them when its dialog unmounts.
    pub fn clear_all(&self, window: &mut Window, cx: &mut gpui_kit::App) {
        for field in [&self.url, &self.root_path, &self.tls_name] {
            field.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.clear_secrets(window, cx);
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
    /// The connection dialog is open, on the surface `host` names.
    pub(super) dialog: bool,
    pub(super) profiles: Vec<ArgoConnectionStatus>,
    pub(super) selected: Preference,
    pub(super) editing: Option<ArgoConnectionProfile>,
    pub(super) scope: String,
    pub(super) host: DialogHost,
    /// The GitOps page dialog's choices. Leaving the page or changing the scope resets them.
    pub(super) page: DialogChoices,
    /// The detail dialog's choices. Closing the detail or opening another resource resets them.
    pub(super) detail: DialogChoices,
    /// The GitOps page dialog's text. As on the web, each dialog keeps its own.
    pub(super) page_fields: DialogFields,
    /// The detail dialog's text.
    pub(super) detail_fields: DialogFields,
    /// The window that owns the field entities, so an unmounted dialog can empty them.
    pub(super) window_handle: gpui_kit::AnyWindowHandle,
    pub(super) servers: Vec<ArgoServerCapability>,
    pub(super) step: ConnectStep,
    /// A running Disconnect or Forget.
    pub(super) profile_action: Option<InFlight>,
    pub(super) reconnect: Reconnect,
    pub(super) discovery: Option<gpui_kit::Task<()>>,
    pub(super) discovery_error: Option<String>,
    pub(super) picker: Entity<
        gpui_kit::component::select::SelectState<
            gpui_kit::component::select::SearchableVec<gpui_kit::SharedString>,
        >,
    >,
    /// Focus container of the connection dialog. Its focus trap keeps Tab and
    /// Shift-Tab inside the dialog, as the web's dialog does.
    pub(super) dialog_focus: FocusHandle,
    pub(super) _subscription: gpui_kit::Subscription,
}
impl Connections {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
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
                        .argo
                        .connections
                        .profiles
                        .iter()
                        .filter_map(|s| s.profile.as_ref())
                        .find(|p| {
                            this.argo_profile_matches(p)
                                && super::profiles::endpoint_label(&p.endpoint) == label
                        })
                    else {
                        return;
                    };
                    Preference::Connected(profile.id.clone())
                }
            };
            this.argo.connections.selected = preference;
            this.persist_argo_preferences(cx);
            this.reload_argo(window, cx);
        });
        Self {
            picker,
            dialog_focus: cx.focus_handle(),
            _subscription: subscription,
            dialog: false,
            profiles: vec![],
            selected: Preference::Automatic,
            editing: None,
            scope: String::new(),
            host: DialogHost::default(),
            page: DialogChoices::default(),
            detail: DialogChoices::default(),
            page_fields: DialogFields::new(window, cx),
            detail_fields: DialogFields::new(window, cx),
            window_handle: window.window_handle(),
            servers: vec![],
            step: ConnectStep::Idle,
            profile_action: None,
            reconnect: Reconnect::default(),
            discovery: None,
            discovery_error: None,
        }
    }
    /// The open dialog's choices.
    pub(super) fn choices(&self) -> &DialogChoices {
        match self.host {
            DialogHost::Detail => &self.detail,
            DialogHost::Page => &self.page,
        }
    }
    pub(super) fn choices_mut(&mut self) -> &mut DialogChoices {
        match self.host {
            DialogHost::Detail => &mut self.detail,
            DialogHost::Page => &mut self.page,
        }
    }
    /// The open dialog's text fields.
    pub(super) fn fields(&self) -> &DialogFields {
        self.fields_for(self.host)
    }
    pub(super) fn fields_for(&self, host: DialogHost) -> &DialogFields {
        match host {
            DialogHost::Detail => &self.detail_fields,
            DialogHost::Page => &self.page_fields,
        }
    }
    /// Forgets what the user chose and typed in one dialog, once the surface
    /// that hosts it goes away.
    pub(super) fn reset_dialog(&mut self, host: DialogHost, cx: &mut gpui_kit::App) {
        match host {
            DialogHost::Detail => self.detail = DialogChoices::default(),
            DialogHost::Page => self.page = DialogChoices::default(),
        }
        self.discard_fields(host, cx);
    }
    /// Empties a dialog's text once its surface unmounts, as the web does. The reset sites
    /// have no window, so the clear runs deferred on the window that owns the entities.
    pub(super) fn discard_fields(&self, host: DialogHost, cx: &mut gpui_kit::App) {
        let fields = self.fields_for(host).clone();
        let handle = self.window_handle;
        cx.defer(move |cx| {
            // A closed window has no fields left to empty.
            let _ = cx.update_window(handle, |_, window, cx| fields.clear_all(window, cx));
        });
    }
    /// The dialog blocks new connects and profile actions while one runs.
    pub(super) fn busy(&self) -> bool {
        matches!(self.step, ConnectStep::Busy { .. }) || self.profile_action.is_some()
    }
}
/// The endpoint the web's server radios offer: a connectable Service tunnel.
fn connectable_endpoint(server: &ArgoServerCapability) -> Option<ArgoServerEndpoint> {
    if server.unavailable_reason.is_some() {
        return None;
    }
    match &server.endpoint {
        Some(endpoint @ ArgoServerEndpoint::ServiceTunnel { .. }) => Some(endpoint.clone()),
        _ => None,
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
        self.argo.connections.fields().clear_secrets(window, cx);
    }
    /// The open dialog's typed `token` or `password`. Masked inputs expose no accessibility
    /// value, so UI tests ask here.
    #[doc(hidden)]
    pub fn argo_typed_secret(&self, field: &str, cx: &gpui_kit::App) -> String {
        let fields = self.argo.connections.fields();
        let input = match field {
            "token" => &fields.token,
            "password" => &fields.password,
            other => panic!("{other} is not a masked Argo CD connection field"),
        };
        input.read(cx).value().to_string()
    }
    /// Applies the web's server-source default to the open dialog. Until the user picks a source,
    /// the first connectable discovered Service is chosen. A chosen Service stays chosen while
    /// discovery still lists it. Otherwise it falls back to the first one, as the web does.
    /// Empty discovery leaves the choice alone, so a read still in flight cannot clear it.
    pub(super) fn settle_argo_source(&mut self) {
        let state = &self.argo.connections;
        if state.servers.is_empty() {
            return;
        }
        let connectable = || state.servers.iter().filter_map(connectable_endpoint);
        let first = connectable().next();
        let choices = state.choices();
        let next = match &choices.endpoint {
            _ if !choices.source_chosen => first,
            Some(chosen) if !connectable().any(|server| same_server(chosen, &server)) => first,
            kept => kept.clone(),
        };
        self.argo.connections.choices_mut().endpoint = next;
    }
    /// Closes the dialog and cancels its preview or connect. A failure stays visible. The typed
    /// text stays too, as the web keeps it until the dialog unmounts or a submit clears it.
    pub(super) fn cancel_argo_connection(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.argo.connections;
        if !matches!(state.step, ConnectStep::Failed { .. }) {
            state.step = ConnectStep::Idle;
        }
        state.discovery.take();
        state.dialog = false;
        state.editing = None;
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
            .argo
            .connections
            .profiles
            .iter_mut()
            .find(|s| s.profile.as_ref().map(|p| &p.id) == id.as_ref())
        {
            *existing = status;
        } else {
            self.argo.connections.profiles.push(status);
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
        if self.argo.connections.busy() {
            return;
        }
        let state = &self.argo.connections;
        let choices = state.choices();
        let fields = state.fields();
        let remembered = state
            .editing
            .as_ref()
            .is_some_and(|p| p.remember_credential);
        let credential = if choices.local {
            !fields.username.read(cx).value().is_empty()
                && !fields.password.read(cx).value().is_empty()
        } else {
            !fields.token.read(cx).value().is_empty()
        };
        if !credential && !remembered {
            self.argo.connections.step = ConnectStep::failed(
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
                    .or_else(|| choices.endpoint.clone())
            })
            .unwrap_or_else(|| ArgoServerEndpoint::ExternalHttps {
                url: fields.url.read(cx).value().trim().to_owned(),
            });
        if confirmed_target.is_none()
            && state.editing.is_none()
            && let ArgoServerEndpoint::ServiceTunnel {
                root_path,
                tls_server_name,
                ..
            } = &mut endpoint
        {
            let root = fields.root_path.read(cx).value().trim().to_owned();
            let name = fields.tls_name.read(cx).value().trim().to_owned();
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
        let url = super::profiles::endpoint_url(&endpoint);
        let external = matches!(endpoint, ArgoServerEndpoint::ExternalHttps { .. });
        let profile = ArgoConnectionProfile {
            id: state.editing.as_ref().map_or_else(
                || {
                    format!(
                        "argo:{}:{}:{}:{}",
                        self.argo_workspace_id(),
                        self.scope.context,
                        self.argo_source_key().unwrap_or("global"),
                        super::profiles::endpoint_identity(&endpoint)
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
            remember_credential: choices.remember
                || state
                    .editing
                    .as_ref()
                    .is_some_and(|p| p.remember_credential),
        };
        let value = |field: &Entity<InputState>| {
            let s = field.read(cx).value().to_string();
            (!s.is_empty()).then_some(s)
        };
        // A remembered profile reconnects with its keyring credential and default TLS
        // checks, as the web does. The form holds what was typed for a new connection,
        // which may be meant for another server.
        let form = !remembered;
        let read = self.backend.connect_argo(ArgoConnect {
            profile,
            token: if form && !choices.local {
                value(&fields.token)
            } else {
                None
            },
            username: if form && choices.local {
                value(&fields.username)
            } else {
                None
            },
            password: if form && choices.local {
                value(&fields.password)
            } else {
                None
            },
            insecure_tls: form && external && choices.insecure,
            custom_ca_pem: if form && external {
                let ca = fields.ca.read(cx).value().to_string();
                (!ca.is_empty()).then(|| ca.into_bytes())
            } else {
                None
            },
            confirmed_target,
        });
        let reconfirm = state.editing.clone().filter(|p| {
            p.remember_credential && matches!(p.endpoint, ArgoServerEndpoint::ServiceTunnel { .. })
        });
        if form {
            self.clear_argo_secrets(window, cx);
        }
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match connected(result) {
                    Ok(status) => {
                        this.argo.connections.step = ConnectStep::Idle;
                        this.record_argo_status(status, window, cx);
                        this.argo.connections.dialog = false;
                        this.reload_argo(window, cx);
                    }
                    Err(message) => {
                        this.argo.connections.step = ConnectStep::Failed { message, reconfirm }
                    }
                }
                cx.notify();
            });
        });
        self.argo.connections.step = ConnectStep::Busy { _read: task.into() };
        cx.notify();
    }
    pub(super) fn disconnect_argo_profile(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.argo.connections.busy() {
            return;
        }
        let read = self.backend.disconnect_argo(id.clone());
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.argo.connections.profile_action = None;
                match result {
                    Ok(()) => {
                        for status in &mut this.argo.connections.profiles {
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
                    Err(error) => this.argo.connections.step = ConnectStep::failed(error.message),
                }
                cx.notify();
            });
        });
        self.argo.connections.step = ConnectStep::Idle;
        self.argo.connections.profile_action = Some(task.into());
        cx.notify();
    }
}
