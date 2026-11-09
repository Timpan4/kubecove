//! Only endpoint metadata and the inspection choice enter ADR 0030 storage.
use super::{
    Inspector,
    argo_connection::{ConnectStep, Preference, Reconnect, connected},
};
use crate::backend::ArgoConnect;
use crate::preferences::{ArgoProfile, ConnectionPreference, Endpoint};
use gpui_kit::{Context, Window};
use kubecove_lib::models::{
    ArgoConfirmedTarget, ArgoConnectionProfile, ArgoConnectionStatus, ArgoServerEndpoint,
};

pub(super) fn endpoint_label(endpoint: &ArgoServerEndpoint) -> String {
    match endpoint {
        ArgoServerEndpoint::ExternalHttps { url } => url.clone(),
        ArgoServerEndpoint::ServiceTunnel {
            namespace,
            service_name,
            service_port,
            scheme,
            ..
        } => format!(
            "{namespace}/{service_name}:{service_port} ({})",
            scheme.to_uppercase()
        ),
    }
}
pub(super) fn endpoint_url(endpoint: &ArgoServerEndpoint) -> String {
    match endpoint {
        ArgoServerEndpoint::ExternalHttps { url } => url.clone(),
        ArgoServerEndpoint::ServiceTunnel {
            namespace,
            service_name,
            scheme,
            root_path,
            ..
        } => format!(
            "{scheme}://{service_name}.{namespace}.svc{}",
            root_path.as_deref().unwrap_or_default()
        ),
    }
}
pub(super) fn endpoint_identity(endpoint: &ArgoServerEndpoint) -> String {
    match endpoint {
        ArgoServerEndpoint::ExternalHttps { url } => url.trim().to_lowercase(),
        ArgoServerEndpoint::ServiceTunnel {
            namespace,
            service_name,
            service_port,
            scheme,
            root_path,
            tls_server_name,
        } => format!(
            "serviceTunnel:{}:{}:{service_port}:{scheme}:{}:{}",
            namespace.trim().to_lowercase(),
            service_name.trim().to_lowercase(),
            root_path
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("/"),
            tls_server_name
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_lowercase()
        ),
    }
}
impl From<Endpoint> for ArgoServerEndpoint {
    fn from(value: Endpoint) -> Self {
        match value {
            Endpoint::ExternalHttps { url } => Self::ExternalHttps { url },
            Endpoint::ServiceTunnel {
                namespace,
                service_name,
                service_port,
                scheme,
                root_path,
                tls_server_name,
            } => Self::ServiceTunnel {
                namespace,
                service_name,
                service_port,
                scheme,
                root_path,
                tls_server_name,
            },
        }
    }
}
impl From<ArgoServerEndpoint> for Endpoint {
    fn from(value: ArgoServerEndpoint) -> Self {
        match value {
            ArgoServerEndpoint::ExternalHttps { url } => Self::ExternalHttps { url },
            ArgoServerEndpoint::ServiceTunnel {
                namespace,
                service_name,
                service_port,
                scheme,
                root_path,
                tls_server_name,
            } => Self::ServiceTunnel {
                namespace,
                service_name,
                service_port,
                scheme,
                root_path,
                tls_server_name,
            },
        }
    }
}
impl Inspector {
    pub(super) fn forget_argo_profile(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.argo_connections.busy() {
            return;
        }
        let Some(profile) = self
            .argo_connections
            .profiles
            .iter()
            .filter_map(|s| s.profile.as_ref())
            .find(|p| p.id == id && self.argo_profile_matches(p))
            .cloned()
        else {
            return;
        };
        // Persist the auto-reconnect opt-out before touching the credential. A later
        // failure leaves a visible, retryable profile that cannot reconnect on startup.
        let mut document = self.persisted.clone();
        for p in &mut document.preferences.argo_profiles {
            if p.id == id {
                p.remember_credential = false;
            }
        }
        match self.backend.save_preferences(document) {
            Ok(document) => self.persisted = document,
            Err(error) => {
                self.argo_connections.step = ConnectStep::failed(format!(
                    "Forget stopped before credential deletion. {}",
                    error.message
                ));
                cx.notify();
                return;
            }
        }
        self.argo_connections.reconnect.queue.retain(|p| p.id != id);
        for p in self
            .argo_connections
            .profiles
            .iter_mut()
            .filter_map(|s| s.profile.as_mut())
        {
            if p.id == id {
                p.remember_credential = false;
            }
        }
        let backend = self.backend.clone();
        let read = async move {
            backend.disconnect_argo(profile.id.clone()).await?;
            backend.forget_argo(profile).await
        };
        // Forget is not cancelled by closing the form. Its credential and metadata
        // cleanup must finish together.
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.argo_connections.profile_action = None;
                for s in &mut this.argo_connections.profiles {
                    if s.profile.as_ref().is_some_and(|p| p.id == id) { s.connected = false; }
                }
                let result = result.and_then(|()| {
                    let mut document = this.persisted.clone();
                    document.preferences.argo_profiles.retain(|p| p.id != id);
                    document.preferences.argo_connection_preferences.retain(|_, p|
                        !matches!(p, ConnectionPreference::Connected { profile_id } if profile_id == &id));
                    this.backend.save_preferences(document)
                });
                match result {
                    Ok(document) => {
                        this.persisted = document;
                        this.argo_connections.profiles.retain(|s| s.profile.as_ref().is_none_or(|p| p.id != id));
                        if matches!(&this.argo_connections.selected, Preference::Connected(p) if p == &id) {
                            this.argo_connections.selected = Preference::Automatic;
                        }
                    }
                    Err(error) => this.argo_connections.step = ConnectStep::failed(format!("Forget incomplete. Automatic reconnect is disabled. Retry Forget. {}", error.message)),
                }
                this.sync_argo_picker(window, cx);
                this.reload_argo(window, cx);
                cx.notify();
            });
        });
        self.argo_connections.step = ConnectStep::Idle;
        self.argo_connections.profile_action = Some(task.into());
        cx.notify();
    }
    pub(super) fn sync_argo_scope(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scope = format!(
            "{}:{}:{:?}",
            self.argo_workspace_id(),
            self.scope.context,
            self.argo_source_key()
        );
        if self.argo_connections.scope != scope {
            self.cancel_argo_connection(window, cx);
            self.argo_connections.discovery.take();
            self.argo_connections.servers.clear();
            self.argo_connections.scope = scope;
            self.argo_connections.step = ConnectStep::Idle;
            // A new scope mounts new web dialogs, which start from their default choices.
            self.argo_connections.page = Default::default();
            self.argo_connections.detail = Default::default();
            self.restore_argo_preferences();
            self.sync_argo_picker(window, cx);
            self.argo_outcomes.clear();
            self.argo_connections.reconnect = Reconnect {
                queue: self
                    .argo_connections
                    .profiles
                    .iter()
                    .filter(|s| !s.connected)
                    .filter_map(|s| s.profile.as_ref())
                    .filter(|p| p.remember_credential && self.argo_profile_matches(p))
                    .cloned()
                    .collect(),
                current: None,
            };
            self.reconnect_next_argo_profile(window, cx);
        }
    }
    pub(super) fn argo_workspace_id(&self) -> String {
        self.launcher
            .active
            .map_or_else(|| "native-session".into(), |id| id.to_string())
    }
    pub(super) fn argo_source_key(&self) -> Option<&str> {
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
        self.argo_connections
            .profiles
            .iter()
            .filter(|s| {
                s.connected || matches!(self.argo_connections.selected, Preference::Connected(_))
            })
            .filter_map(|s| s.profile.as_ref())
            .find(|p| {
                self.argo_profile_matches(p)
                    && match &self.argo_connections.selected {
                        Preference::Connected(id) => id == &p.id,
                        Preference::Automatic => true,
                        Preference::Kubernetes => false,
                    }
            })
            .map(|p| p.id.clone())
    }
    /// Why the selected connection cannot serve operations, as the web's
    /// `argoOperationAvailability`. Only a saved preference for one profile can be unavailable.
    pub(super) fn argo_connection_unavailable(&self) -> Option<String> {
        const FALLBACK: &str = "Selected Argo CD provider connection is unavailable.";
        let Preference::Connected(id) = &self.argo_connections.selected else {
            return None;
        };
        let status = self
            .argo_connections
            .profiles
            .iter()
            .find(|s| s.profile.as_ref().is_some_and(|p| &p.id == id));
        match status {
            Some(status) if status.connected => None,
            Some(status) => Some(
                status
                    .unavailable_reason
                    .clone()
                    .unwrap_or_else(|| FALLBACK.into()),
            ),
            None => Some(FALLBACK.into()),
        }
    }
    pub(super) fn argo_profile_matches(&self, p: &ArgoConnectionProfile) -> bool {
        p.cluster_context.as_deref() == Some(&self.scope.context)
            && p.workspace_id.as_deref() == Some(&self.argo_workspace_id())
            && p.kubeconfig_source_key.as_deref() == self.argo_source_key()
    }

    /// Reconnects the next remembered profile, as the web's `reconnectRememberedArgoProfile`.
    /// It leaves the dialog alone, and a failed attempt waits silently for a manual reconnect.
    fn reconnect_next_argo_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(saved) = self.argo_connections.reconnect.queue.pop_front() else {
            self.argo_connections.reconnect.current = None;
            return;
        };
        let confirmed_target = match &saved.endpoint {
            ArgoServerEndpoint::ServiceTunnel {
                namespace,
                service_name,
                ..
            } => Some(ArgoConfirmedTarget {
                namespace: namespace.clone(),
                service_name: service_name.clone(),
                pod_name: None,
                certificate_sha256: None,
            }),
            _ => None,
        };
        let profile = ArgoConnectionProfile {
            url: endpoint_url(&saved.endpoint),
            id: saved.id,
            endpoint: saved.endpoint,
            cluster_context: Some(self.scope.context.clone()),
            workspace_id: Some(self.argo_workspace_id()),
            kubeconfig_source_key: self.argo_source_key().map(str::to_owned),
            transport: "connected".into(),
            remember_credential: true,
        };
        let read = self.backend.connect_argo(ArgoConnect {
            profile,
            token: None,
            username: None,
            password: None,
            insecure_tls: false,
            custom_ca_pem: None,
            confirmed_target,
        });
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Ok(status) = connected(result) {
                    this.record_argo_status(status, window, cx);
                    this.reload_argo(window, cx);
                }
                this.reconnect_next_argo_profile(window, cx);
                cx.notify();
            });
        });
        self.argo_connections.reconnect.current = Some(task.into());
    }
    pub(super) fn sync_argo_picker(
        &mut self,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_kit::{SharedString, component::select::SearchableVec};
        let mut choices: Vec<SharedString> = vec!["Automatic".into(), "Kubernetes".into()];
        choices.extend(
            self.argo_connections
                .profiles
                .iter()
                .filter_map(|s| s.profile.as_ref())
                .filter(|p| self.argo_profile_matches(p))
                .map(|p| endpoint_label(&p.endpoint).into()),
        );
        let selected: SharedString = match &self.argo_connections.selected {
            Preference::Automatic => "Automatic".into(),
            Preference::Kubernetes => "Kubernetes".into(),
            Preference::Connected(id) => self
                .argo_connections
                .profiles
                .iter()
                .filter_map(|s| s.profile.as_ref())
                .find(|p| &p.id == id)
                .map_or_else(|| id.clone().into(), |p| endpoint_label(&p.endpoint).into()),
        };
        self.argo_connections.picker.update(cx, |picker, cx| {
            picker.set_items(SearchableVec::new(choices), window, cx);
            picker.set_selected_value(&selected, window, cx);
        });
    }
    fn argo_preference_key(&self) -> String {
        self.argo_workspace_id()
    }
    pub(super) fn restore_argo_preferences(&mut self) {
        let mut statuses = std::mem::take(&mut self.argo_connections.profiles);
        self.argo_connections.profiles = self
            .persisted
            .preferences
            .argo_profiles
            .iter()
            .map(|p| {
                if let Some(index) = statuses
                    .iter()
                    .position(|s| s.profile.as_ref().is_some_and(|s| s.id == p.id))
                {
                    return statuses.remove(index);
                }
                let endpoint = ArgoServerEndpoint::from(p.endpoint.clone());
                ArgoConnectionStatus {
                    profile: Some(ArgoConnectionProfile {
                        id: p.id.clone(),
                        url: endpoint_url(&endpoint),
                        endpoint,
                        cluster_context: p.cluster_context.clone(),
                        workspace_id: p.workspace_id.clone(),
                        kubeconfig_source_key: p.kubeconfig_source_key.clone(),
                        transport: "connected".into(),
                        remember_credential: p.remember_credential,
                    }),
                    connected: false,
                    ..Default::default()
                }
            })
            .collect();
        self.argo_connections.selected = match self
            .persisted
            .preferences
            .argo_connection_preferences
            .get(&self.argo_preference_key())
        {
            Some(ConnectionPreference::Kubernetes) => Preference::Kubernetes,
            Some(ConnectionPreference::Connected { profile_id }) => {
                Preference::Connected(profile_id.clone())
            }
            _ => Preference::Automatic,
        };
    }
    pub(super) fn persist_argo_preferences(&mut self, cx: &mut Context<Self>) {
        self.persisted.preferences.argo_profiles = self
            .argo_connections
            .profiles
            .iter()
            .filter_map(|s| s.profile.as_ref())
            .map(|p| ArgoProfile {
                id: p.id.clone(),
                endpoint: p.endpoint.clone().into(),
                cluster_context: p.cluster_context.clone(),
                workspace_id: p.workspace_id.clone(),
                kubeconfig_source_key: p.kubeconfig_source_key.clone(),
                remember_credential: p.remember_credential,
            })
            .collect();
        let preference = match &self.argo_connections.selected {
            Preference::Automatic => ConnectionPreference::Automatic,
            Preference::Kubernetes => ConnectionPreference::Kubernetes,
            Preference::Connected(id) => ConnectionPreference::Connected {
                profile_id: id.clone(),
            },
        };
        self.persisted
            .preferences
            .argo_connection_preferences
            .insert(self.argo_preference_key(), preference);
        self.save_preferences(cx);
    }
}
