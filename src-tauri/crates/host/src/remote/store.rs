//! The selected remote server, its session state, and desktop sign-ins
//! (ADR 0031). Sign-ins live in Rust memory and native credential storage,
//! never in a file, view state or command result.
use super::client::RemoteConnection;
use super::models::{RemoteAuthMode, RemoteConnectionSummary, RemoteSessionState};
use super::sign_in::{BrowserLauncher, SystemBrowser};
use kubecove_gitops::commands::argo::{KeyringStore, SecureStore};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{RwLock, oneshot};

/// Native credential storage service for desktop sign-ins, keyed by the
/// normalized server URL.
pub(super) const SIGN_IN_SERVICE: &str = "KubeCove remote sign-in";

pub struct RemoteConnectionStore {
    pub(super) current: RwLock<Option<Arc<RemoteConnection>>>,
    pub(super) selection: Mutex<Option<Selection>>,
    pub(super) sign_ins: SignIns,
    pub(super) browser: Arc<dyn BrowserLauncher>,
}

impl Default for RemoteConnectionStore {
    fn default() -> Self {
        Self::new(Arc::new(KeyringStore), Arc::new(SystemBrowser))
    }
}

impl RemoteConnectionStore {
    /// Tests pass an in-memory store and a launcher that opens no browser.
    pub fn new(secure: Arc<dyn SecureStore>, browser: Arc<dyn BrowserLauncher>) -> Self {
        Self {
            current: RwLock::new(None),
            selection: Mutex::new(None),
            sign_ins: SignIns {
                secure,
                memory: Mutex::new(HashMap::new()),
            },
            browser,
        }
    }

    pub(super) fn selection(&self) -> Option<Selection> {
        self.selection
            .lock()
            .expect("remote selection lock")
            .clone()
    }

    /// Updates the selection only while it still names `server_url`, so a
    /// sign-in that finishes after Disconnect changes nothing.
    pub(super) fn update_selection(
        &self,
        server_url: &str,
        change: impl FnOnce(&mut Selection),
    ) -> bool {
        let mut selection = self.selection.lock().expect("remote selection lock");
        match selection.as_mut() {
            Some(selected) if selected.server_url == server_url => {
                change(selected);
                true
            }
            _ => false,
        }
    }

    /// The selected remote, with the open connection's state when there is one.
    /// A refused OIDC sign-in is forgotten here, wherever it was noticed.
    pub(super) async fn summary(&self) -> Option<RemoteConnectionSummary> {
        let connection = self.current.read().await.clone();
        let selection = self.selection();
        let summary = match (connection, &selection) {
            (Some(connection), _) => {
                let mut summary = connection.summary.clone();
                summary.session_state = connection.session_state();
                summary.connected = summary.session_state == RemoteSessionState::Connected
                    || summary.session_state == RemoteSessionState::Reconnecting;
                summary
            }
            (None, Some(selected)) => RemoteConnectionSummary::selected(
                selected.server_url.clone(),
                selected.auth_mode,
                selected.state,
            ),
            (None, None) => return None,
        };
        let mut summary = summary;
        summary.sign_in_memory_only = selection
            .as_ref()
            .is_some_and(|selected| selected.memory_only);
        if summary.auth_mode == RemoteAuthMode::Oidc
            && summary.session_state == RemoteSessionState::SignInExpired
        {
            self.sign_ins.forget_refused(&summary.server_url);
        }
        Some(summary)
    }
}

#[derive(Clone)]
pub(super) struct Selection {
    pub server_url: String,
    pub auth_mode: RemoteAuthMode,
    /// The state while no connection is open.
    pub state: RemoteSessionState,
    pub memory_only: bool,
    pub pending: Option<Arc<PendingSignIn>>,
}

impl Selection {
    pub(super) fn new(
        server_url: String,
        auth_mode: RemoteAuthMode,
        state: RemoteSessionState,
    ) -> Self {
        Self {
            server_url,
            auth_mode,
            state,
            memory_only: false,
            pending: None,
        }
    }
}

/// A browser sign-in waiting for its loopback redirect.
pub(super) struct PendingSignIn {
    pub link: String,
    pub cancel: Mutex<Option<oneshot::Sender<()>>>,
}

impl PendingSignIn {
    pub(super) fn cancel(&self) {
        if let Some(cancel) = self.cancel.lock().expect("sign-in cancel lock").take() {
            let _ = cancel.send(());
        }
    }
}

pub(super) struct SignIns {
    secure: Arc<dyn SecureStore>,
    memory: Mutex<HashMap<String, String>>,
}

impl SignIns {
    /// The held sign-in for `server_url`, read from native storage only when
    /// memory has none.
    pub(super) fn get(&self, server_url: &str) -> Option<String> {
        let mut memory = self.memory.lock().expect("remote sign-in lock");
        if let Some(credential) = memory.get(server_url) {
            return Some(credential.clone());
        }
        let stored = self
            .secure
            .read(SIGN_IN_SERVICE, server_url)
            .ok()
            .flatten()?;
        memory.insert(server_url.to_owned(), stored.clone());
        Some(stored)
    }

    /// Keeps the sign-in and returns whether it is in memory only, because
    /// native storage refused it.
    pub(super) fn put(&self, server_url: &str, credential: String) -> bool {
        let memory_only = self
            .secure
            .write(SIGN_IN_SERVICE, server_url, &credential)
            .is_err();
        self.memory
            .lock()
            .expect("remote sign-in lock")
            .insert(server_url.to_owned(), credential);
        memory_only
    }

    /// Drops the sign-in from memory and native storage, whether or not the
    /// server call that ended it succeeded.
    pub(super) fn forget(&self, server_url: &str) {
        self.memory
            .lock()
            .expect("remote sign-in lock")
            .remove(server_url);
        let _ = self.secure.delete(SIGN_IN_SERVICE, server_url);
    }

    /// Forgets a sign-in the server refused. Native storage is asked only
    /// when memory held it, so repeated status reads stay local.
    pub(super) fn forget_refused(&self, server_url: &str) {
        let held = self
            .memory
            .lock()
            .expect("remote sign-in lock")
            .remove(server_url)
            .is_some();
        if held {
            let _ = self.secure.delete(SIGN_IN_SERVICE, server_url);
        }
    }

    #[cfg(test)]
    pub(super) fn in_memory(&self, server_url: &str) -> bool {
        self.memory
            .lock()
            .expect("remote sign-in lock")
            .contains_key(server_url)
    }
}
