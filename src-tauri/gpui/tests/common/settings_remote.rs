//! The remote server and its OIDC sign-in, as `src/lib/settings-integration-mocks.ts`
//! scripts them for the web driver. No browser, keyring or server is reached.
use super::Fixture;
use kubecove_gpui::backend::{Read, RemoteConnectionSummary};
use kubecove_host::remote::{RemoteAuthMode, RemoteSessionState};
use kubecove_lib::models::{AppError, AppErrorKind};
use std::sync::atomic::Ordering;

const SIGN_IN_LINK: &str = "http://127.0.0.1:43117/api/v1/oidc/native/start?redirect_uri=http%3A%2F%2F127.0.0.1%3A49152%2Fkubecove%2Fcallback";

fn summary(server_url: String, auth_mode: RemoteAuthMode) -> RemoteConnectionSummary {
    RemoteConnectionSummary {
        server_url,
        server_version: "0.13.0".into(),
        session_grace_seconds: 300,
        connected: true,
        credential_store: Some("encryptedFile".into()),
        auth_mode,
        session_state: RemoteSessionState::Connected,
        sign_in_memory_only: false,
        sign_in_not_deleted: false,
    }
}

fn ready<T: Send + 'static>(value: Result<T, AppError>) -> Read<T> {
    Box::pin(async move { value })
}

// The fixture also compiles into native and visual tests without parity actions.
#[allow(dead_code)]
impl Fixture {
    /// The selected server stops answering. It stays selected, its commands
    /// fail as disconnected, and the grace window has elapsed.
    pub fn drop_remote_connection(&self) {
        self.remote_signal(RemoteSessionState::SessionExpired);
    }
    /// The state a server signal leaves the selected remote in.
    pub fn remote_signal(&self, state: RemoteSessionState) {
        let mut remote = self.settings.remote.lock().unwrap();
        let Some(selected) = remote.as_mut() else {
            return;
        };
        selected.session_state = state;
        selected.connected = state == RemoteSessionState::Reconnecting;
        if state == RemoteSessionState::SignInExpired {
            self.settings
                .sign_ins
                .lock()
                .unwrap()
                .remove(&selected.server_url);
        }
        drop(remote);
        self.settings.remote_failures.fetch_add(1, Ordering::AcqRel);
        self.sync_remote_refusal();
    }
    pub fn runtime_endpoint(&self) -> String {
        self.settings
            .remote
            .lock()
            .unwrap()
            .as_ref()
            .map_or_else(|| "local".to_owned(), |remote| remote.server_url.clone())
    }
    pub(super) fn remote_failures_read(&self) -> u64 {
        self.settings.remote_failures.load(Ordering::Acquire)
    }
    /// Commands fail while a selected remote is not connected, as
    /// `kubecove_host::remote` refuses them, so nothing falls back to local data.
    fn sync_remote_refusal(&self) {
        let disconnected = self
            .settings
            .remote
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|remote| !remote.connected);
        self.timing.refuse(disconnected.then(|| {
            AppError::new(
                "connect to a remote KubeCove server first",
                AppErrorKind::Session,
            )
        }));
    }
    fn set_remote(
        &self,
        remote: Option<RemoteConnectionSummary>,
    ) -> Option<RemoteConnectionSummary> {
        self.settings.remote.lock().unwrap().clone_from(&remote);
        self.sync_remote_refusal();
        remote
    }
    pub(super) fn remote_status_read(&self) -> Read<Option<RemoteConnectionSummary>> {
        ready(Ok(self.settings.remote.lock().unwrap().clone()))
    }
    pub(super) fn connect_remote_read(
        &self,
        url: String,
        token: String,
    ) -> Read<RemoteConnectionSummary> {
        if !token.trim().is_empty() {
            let selected = summary(url, RemoteAuthMode::Token);
            self.set_remote(Some(selected.clone()));
            return ready(Ok(selected));
        }
        let script = self.settings.script.lock().unwrap().clone();
        if script == "access-server" {
            return ready(Err(AppError::new(
                "Desktop sign-in for Cloudflare Access is not supported. Use a token or OIDC server.",
                AppErrorKind::UnsupportedOperation,
            )));
        }
        if !script.starts_with("oidc-") {
            return ready(Err(AppError::new(
                "This server uses a server token. Enter the token to connect.",
                AppErrorKind::Validation,
            )));
        }
        let mut selected = summary(url, RemoteAuthMode::Oidc);
        if !self
            .settings
            .sign_ins
            .lock()
            .unwrap()
            .contains(&selected.server_url)
        {
            selected.connected = false;
            selected.credential_store = None;
            selected.session_state = RemoteSessionState::SignInRequired;
        }
        selected.sign_in_memory_only = script == "oidc-memory-only" && selected.connected;
        ready(Ok(self.set_remote(Some(selected)).unwrap()))
    }
    pub(super) fn disconnect_remote_read(&self) -> Read<()> {
        self.set_remote(None);
        ready(Ok(()))
    }
    pub(super) fn remote_sign_in_read(&self) -> Read<Option<RemoteConnectionSummary>> {
        let Some(mut selected) = self.settings.remote.lock().unwrap().clone() else {
            return ready(Ok(None));
        };
        let script = self.settings.script.lock().unwrap().clone();
        if script == "oidc-browser-waits" {
            selected.session_state = RemoteSessionState::WaitingForBrowser;
            selected.connected = false;
        } else {
            self.settings
                .sign_ins
                .lock()
                .unwrap()
                .insert(selected.server_url.clone());
            selected.session_state = RemoteSessionState::Connected;
            selected.connected = true;
            selected.sign_in_memory_only = script == "oidc-memory-only";
            selected.sign_in_not_deleted = false;
        }
        ready(Ok(self.set_remote(Some(selected))))
    }
    pub(super) fn remote_cancel_sign_in_read(&self) -> Read<()> {
        if let Some(selected) = self.settings.remote.lock().unwrap().as_mut()
            && selected.session_state == RemoteSessionState::WaitingForBrowser
        {
            selected.session_state = RemoteSessionState::SignInRequired;
        }
        ready(Ok(()))
    }
    pub(super) fn remote_sign_in_link_read(&self) -> Read<Option<String>> {
        let waiting = self
            .settings
            .remote
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|remote| remote.session_state == RemoteSessionState::WaitingForBrowser);
        ready(Ok(waiting.then(|| SIGN_IN_LINK.to_owned())))
    }
    pub(super) fn remote_start_new_session_read(&self) -> Read<RemoteConnectionSummary> {
        let Some(mut selected) = self.settings.remote.lock().unwrap().clone() else {
            return ready(Err(AppError::new(
                "Select a remote KubeCove server first.",
                AppErrorKind::Session,
            )));
        };
        let held = selected.auth_mode == RemoteAuthMode::Token
            || self
                .settings
                .sign_ins
                .lock()
                .unwrap()
                .contains(&selected.server_url);
        selected.connected = held;
        selected.session_state = if held {
            RemoteSessionState::Connected
        } else {
            RemoteSessionState::SignInExpired
        };
        ready(Ok(self.set_remote(Some(selected)).unwrap()))
    }
    pub(super) fn remote_sign_out_read(&self) -> Read<RemoteConnectionSummary> {
        let Some(mut selected) = self.settings.remote.lock().unwrap().clone() else {
            return ready(Err(AppError::new(
                "Select a server that uses single sign-on first.",
                AppErrorKind::Session,
            )));
        };
        self.settings
            .sign_ins
            .lock()
            .unwrap()
            .remove(&selected.server_url);
        selected.connected = false;
        selected.session_state = RemoteSessionState::SignInRequired;
        selected.sign_in_memory_only = false;
        // The credential store refuses to delete the sign-in.
        selected.sign_in_not_deleted =
            *self.settings.script.lock().unwrap() == "oidc-store-keeps-sign-in";
        ready(Ok(self.set_remote(Some(selected)).unwrap()))
    }
}
