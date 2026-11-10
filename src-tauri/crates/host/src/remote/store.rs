//! The selected remote server, its session state, and desktop sign-ins
//! (ADR 0031). Sign-ins live in Rust memory and native credential storage,
//! never in a file, view state or command result.
use super::client::RemoteConnection;
use super::models::{RemoteAuthMode, RemoteConnectionSummary, RemoteSessionState};
use super::sign_in::{BrowserLauncher, SystemBrowser};
use kubecove_gitops::commands::argo::{KeyringStore, SecureStore};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::{RwLock, oneshot};

/// Native credential storage service for desktop sign-ins, keyed by the
/// normalized server URL.
pub(super) const SIGN_IN_SERVICE: &str = "KubeCove remote sign-in";

/// A new value for each selection, sign-in and sign-out. A session attempt
/// publishes only while the selection still has the generation it started
/// with, so one that finishes after any of them changes nothing.
pub(super) fn next_generation() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

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
                undeleted: Mutex::new(HashSet::new()),
            },
            browser,
        }
    }

    /// Whether a remote server is selected, connected or not. A client routes
    /// by this, so a selection that a failed or cancelled connect committed
    /// never falls back to local credentials.
    pub fn is_selected(&self) -> bool {
        self.selection
            .lock()
            .expect("remote selection lock")
            .is_some()
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

    /// Updates the selection only while it still has `generation`.
    pub(super) fn update_generation(
        &self,
        generation: u64,
        change: impl FnOnce(&mut Selection),
    ) -> bool {
        let mut selection = self.selection.lock().expect("remote selection lock");
        match selection.as_mut() {
            Some(selected) if selected.generation == generation => {
                change(selected);
                true
            }
            _ => false,
        }
    }

    /// The selected remote, with the open connection's state when there is one.
    /// A sign-in an open connection saw refused is forgotten here, wherever it
    /// was noticed.
    pub(super) async fn summary(&self) -> Option<RemoteConnectionSummary> {
        let connection = self.current.read().await.clone();
        let selection = self.selection();
        let refused = connection
            .as_ref()
            .map(|connection| connection.credentials.bearer_token.clone());
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
        // Only the credential this connection used: a sign-in that replaced
        // the connection after the snapshot above is newer and stays.
        if let Some(refused) = refused
            && summary.auth_mode == RemoteAuthMode::Oidc
            && summary.session_state == RemoteSessionState::SignInExpired
        {
            self.sign_ins.forget_refused(&summary.server_url, &refused);
        }
        summary.sign_in_memory_only = summary.auth_mode == RemoteAuthMode::Oidc
            && self.sign_ins.memory_only(&summary.server_url);
        summary.sign_in_not_deleted = self.sign_ins.undeleted(&summary.server_url);
        Some(summary)
    }
}

#[derive(Clone)]
pub(super) struct Selection {
    pub server_url: String,
    pub auth_mode: RemoteAuthMode,
    /// The state while no connection is open.
    pub state: RemoteSessionState,
    pub pending: Option<Arc<PendingSignIn>>,
    /// See [`next_generation`].
    pub generation: u64,
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
            pending: None,
            generation: next_generation(),
        }
    }
}

/// A browser sign-in waiting for its loopback redirect.
pub(super) struct PendingSignIn {
    pub link: String,
    pub cancel: Mutex<Option<oneshot::Sender<()>>>,
}

impl PendingSignIn {
    /// Ends the sign-in: Cancel, Disconnect, Sign out and a new selection all
    /// call this before they act on the held sign-in.
    pub(super) fn cancel(&self) {
        if let Some(cancel) = self.cancel.lock().expect("sign-in cancel lock").take() {
            let _ = cancel.send(());
        }
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancel.lock().expect("sign-in cancel lock").is_none()
    }

    /// Runs `publish` unless the sign-in has ended. `cancel` waits while it
    /// runs, so what `publish` stores is in place before the caller of
    /// `cancel` forgets it.
    pub(super) fn unless_cancelled<T>(&self, publish: impl FnOnce() -> T) -> Option<T> {
        let cancel = self.cancel.lock().expect("sign-in cancel lock");
        cancel.is_some().then(publish)
    }
}

pub(super) struct SignIns {
    secure: Arc<dyn SecureStore>,
    memory: Mutex<HashMap<String, Held>>,
    /// Servers whose stored sign-in native storage could not delete. The
    /// entry is not read again in this process until a new sign-in for that
    /// server is stored over it. Lock after `memory`.
    undeleted: Mutex<HashSet<String>>,
}

/// A sign-in held in memory, and whether native storage refused it, so it
/// lasts only as long as this process.
struct Held {
    credential: String,
    memory_only: bool,
}

impl SignIns {
    /// The held sign-in for `server_url`, read from native storage only when
    /// memory has none and the stored entry was not meant to be deleted.
    pub(super) fn get(&self, server_url: &str) -> Option<String> {
        let mut memory = self.memory.lock().expect("remote sign-in lock");
        if let Some(held) = memory.get(server_url) {
            return Some(held.credential.clone());
        }
        if self.undeleted(server_url) {
            return None;
        }
        let stored = self
            .secure
            .read(SIGN_IN_SERVICE, server_url)
            .ok()
            .flatten()?;
        memory.insert(
            server_url.to_owned(),
            Held {
                credential: stored.clone(),
                memory_only: false,
            },
        );
        Some(stored)
    }

    /// Keeps the sign-in, in native storage when it accepts it. When it
    /// refuses, an older stored sign-in is deleted, because a restart would
    /// otherwise bring it back in place of this one.
    pub(super) fn put(&self, server_url: &str, credential: String) {
        let mut memory = self.memory.lock().expect("remote sign-in lock");
        let memory_only = self
            .secure
            .write(SIGN_IN_SERVICE, server_url, &credential)
            .is_err();
        if !memory_only {
            self.undeleted
                .lock()
                .expect("remote sign-in lock")
                .remove(server_url);
        } else if matches!(self.secure.read(SIGN_IN_SERVICE, server_url), Ok(Some(_))) {
            self.delete_stored(server_url);
        }
        memory.insert(
            server_url.to_owned(),
            Held {
                credential,
                memory_only,
            },
        );
    }

    /// Whether the held sign-in for `server_url` is in memory only.
    pub(super) fn memory_only(&self, server_url: &str) -> bool {
        self.memory
            .lock()
            .expect("remote sign-in lock")
            .get(server_url)
            .is_some_and(|held| held.memory_only)
    }

    /// Drops the sign-in from memory and native storage, whether or not the
    /// server call that ended it succeeded. A failed deletion is recorded:
    /// the entry is not used again, and the summary reports it.
    pub(super) fn forget(&self, server_url: &str) {
        let mut memory = self.memory.lock().expect("remote sign-in lock");
        memory.remove(server_url);
        self.delete_stored(server_url);
    }

    /// Forgets `credential`, which the server refused, while it is still the
    /// held sign-in. Native storage is asked only when memory held it, so
    /// repeated status reads stay local. `connect` trims the bearer it uses.
    pub(super) fn forget_refused(&self, server_url: &str, credential: &str) {
        let mut memory = self.memory.lock().expect("remote sign-in lock");
        let held = memory
            .get(server_url)
            .is_some_and(|held| held.credential.trim() == credential.trim());
        if held {
            memory.remove(server_url);
            self.delete_stored(server_url);
        }
    }

    /// Callers hold `memory`, so no read restores the entry meanwhile.
    fn delete_stored(&self, server_url: &str) {
        let deleted = self.secure.delete(SIGN_IN_SERVICE, server_url).is_ok();
        let mut undeleted = self.undeleted.lock().expect("remote sign-in lock");
        if deleted {
            undeleted.remove(server_url);
        } else {
            undeleted.insert(server_url.to_owned());
        }
    }

    /// Whether native storage still holds a sign-in it could not delete.
    pub(super) fn undeleted(&self, server_url: &str) -> bool {
        self.undeleted
            .lock()
            .expect("remote sign-in lock")
            .contains(server_url)
    }

    #[cfg(test)]
    pub(super) fn in_memory(&self, server_url: &str) -> bool {
        self.memory
            .lock()
            .expect("remote sign-in lock")
            .contains_key(server_url)
    }
}
