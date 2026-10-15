use super::{client_cache::ClientCaches, forwarded_bearer::BearerSlot};
use crate::commands::diagnostics::BackendDiagnosticsState;
use std::{
    future::Future,
    sync::{Arc, LazyLock, Mutex},
};

tokio::task_local! {
    static CURRENT: Arc<SessionScope>;
}

static PROCESS: LazyLock<Arc<SessionScope>> = LazyLock::new(|| SessionScope::new(None));

/// One session's client state: its bearer slot, client caches and their
/// cancellation generation, and its command diagnostics.
///
/// `kubecove-server` runs every command of a session inside that session's
/// scope, so one session's requests and workspace cancellation never reach
/// another's. The desktop app is one session: code outside any scope uses
/// the process scope, through the same path.
pub struct SessionScope {
    bearer: Arc<BearerSlot>,
    pub(crate) caches: ClientCaches,
    pub(crate) diagnostics: Mutex<BackendDiagnosticsState>,
    credential_namespace: Option<String>,
}

impl SessionScope {
    /// `credential_namespace` keeps this session's stored Argo CD credentials
    /// apart from every other namespace's. The server passes the signed-in
    /// subject.
    pub fn new(credential_namespace: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            bearer: Arc::default(),
            caches: ClientCaches::default(),
            diagnostics: Mutex::default(),
            credential_namespace,
        })
    }

    /// Sets the bearer token Kubernetes requests carry while forwarding is
    /// enabled, until the supplied Unix deadline. `None` sends requests without
    /// credentials. Expiry is checked by each request, independent of renewal.
    pub fn set_bearer(&self, token: Option<(&str, u64)>) {
        self.bearer.set(token);
    }

    /// The Authorization value requests carry now, for the server's tests.
    #[doc(hidden)]
    pub fn bearer(&self) -> Option<String> {
        self.bearer
            .get()
            .and_then(|value| value.to_str().ok().map(str::to_owned))
    }

    /// Runs `future` with this scope as the current one.
    pub async fn run<F: Future>(self: Arc<Self>, future: F) -> F::Output {
        CURRENT.scope(self, future).await
    }

    /// Runs `f` with this scope as the current one. State that captures the
    /// scope when it is built, such as the Argo CD review store, must be
    /// built inside it.
    pub fn run_sync<R>(self: Arc<Self>, f: impl FnOnce() -> R) -> R {
        CURRENT.sync_scope(self, f)
    }

    /// The scope the calling task runs in, if any.
    pub fn current() -> Option<Arc<Self>> {
        CURRENT.try_with(Arc::clone).ok()
    }

    /// The current scope, or the process scope outside any.
    pub fn current_or_process() -> Arc<Self> {
        Self::current().unwrap_or_else(|| Arc::clone(&PROCESS))
    }

    pub(crate) fn bearer_slot(&self) -> Arc<BearerSlot> {
        Arc::clone(&self.bearer)
    }

    pub fn credential_namespace(&self) -> Option<&str> {
        self.credential_namespace.as_deref()
    }
}
