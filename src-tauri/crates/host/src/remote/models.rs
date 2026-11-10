use serde::{Deserialize, Serialize};

/// How the selected server authenticates this desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAuthMode {
    /// The server's startup token.
    Token,
    /// OIDC through the system browser (ADR 0031).
    Oidc,
}

/// The selected remote's session state (ADR 0031, "Session notices in the
/// native UI"). The clients choose the notice text from it and the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteSessionState {
    Connected,
    /// An OIDC server is selected and needs a browser sign-in.
    SignInRequired,
    Connecting,
    WaitingForBrowser,
    /// The socket dropped and the client is reconnecting within the grace window.
    Reconnecting,
    /// The server moved or closed this app's session.
    SessionEnded,
    /// The server refused the sign-in credential.
    SignInExpired,
    /// The grace window elapsed without a reconnect.
    SessionExpired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnectionSummary {
    pub server_url: String,
    pub server_version: String,
    pub session_grace_seconds: u64,
    pub connected: bool,
    /// The server's credential store kind, such as `native` or `encryptedFile`.
    /// `None` when the server does not report one.
    pub credential_store: Option<String>,
    pub auth_mode: RemoteAuthMode,
    pub session_state: RemoteSessionState,
    /// The sign-in lasts only until the app closes, because native credential
    /// storage is unavailable.
    pub sign_in_memory_only: bool,
    /// Native credential storage could not delete an ended sign-in for this
    /// server. This app does not use it again until a new sign-in replaces it.
    #[serde(default)]
    pub sign_in_not_deleted: bool,
}

impl RemoteConnectionSummary {
    /// A selected server without an open session.
    pub(super) fn selected(
        server_url: String,
        auth_mode: RemoteAuthMode,
        session_state: RemoteSessionState,
    ) -> Self {
        Self {
            server_url,
            server_version: String::new(),
            session_grace_seconds: 0,
            connected: false,
            credential_store: None,
            auth_mode,
            session_state,
            sign_in_memory_only: false,
            sign_in_not_deleted: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSession {
    pub resume_token: String,
    pub session_grace_seconds: u64,
    pub server_version: String,
    #[serde(default)]
    pub credential_store: Option<String>,
}

impl std::fmt::Debug for ServerSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServerSession")
            .field("resume_token", &"[REDACTED]")
            .field("session_grace_seconds", &self.session_grace_seconds)
            .field("server_version", &self.server_version)
            .field("credential_store", &self.credential_store)
            .finish()
    }
}

pub struct RemoteCredentials {
    pub server_url: String,
    pub bearer_token: String,
    pub resume_token: String,
}

impl std::fmt::Debug for RemoteCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RemoteCredentials")
            .field("server_url", &self.server_url)
            .field("bearer_token", &"[REDACTED]")
            .field("resume_token", &"[REDACTED]")
            .finish()
    }
}
