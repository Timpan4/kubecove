//! Local credential plugin failures (ADR 0037). Rust attaches them only for
//! the desktop user's own kubeconfig; server sessions never build an identity.

use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, sync::Arc};

/// What a context's credential plugin is, known when its client is built.
/// Holds no plugin arguments beyond leading subcommand words, no environment
/// and no output.
#[derive(Clone, PartialEq, Eq)]
pub struct CredentialIdentity {
    pub context: String,
    pub user: Option<String>,
    pub command: String,
    pub kubeconfig: Vec<String>,
    pub reproduce: String,
}

/// A failed credential plugin, as the frontend shows it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialFailure {
    pub context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    pub command: String,
    pub kubeconfig: Vec<String>,
    /// A terminal command that runs the same sign-in through kubectl.
    pub reproduce: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_status: Option<i32>,
    /// The plugin's last stderr line, redacted in Rust before serialization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_message: Option<String>,
}

// Logs and Debug output never carry plugin output.
impl fmt::Debug for CredentialFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialFailure")
            .field("context", &self.context)
            .field("command", &self.command)
            .field("exit_status", &self.exit_status)
            .finish_non_exhaustive()
    }
}

impl CredentialIdentity {
    pub(crate) fn failure(&self, error: &kube::client::AuthError) -> CredentialFailure {
        use kube::client::AuthError;
        let (exit_status, plugin_message) = match error {
            AuthError::AuthExecRun { out, .. } => (
                out.status.code(),
                crate::redaction::plugin_error_line(&out.stderr),
            ),
            _ => (None, None),
        };
        CredentialFailure {
            context: self.context.clone(),
            user: self.user.clone(),
            command: self.command.clone(),
            kubeconfig: self.kubeconfig.clone(),
            reproduce: self.reproduce.clone(),
            exit_status,
            plugin_message,
        }
    }
}

/// Tags a request's authentication failure with its plugin identity, so
/// `AppError::from(kube::Error)` can describe a token refresh that fails
/// after the client was built.
pub struct CredentialPluginError {
    identity: Arc<CredentialIdentity>,
    source: Box<dyn Error + Send + Sync>,
}

impl CredentialPluginError {
    pub fn new(identity: Arc<CredentialIdentity>, source: Box<dyn Error + Send + Sync>) -> Self {
        Self { identity, source }
    }

    pub(crate) fn identity(&self) -> &CredentialIdentity {
        &self.identity
    }
}

// The wrapped kube error formats plugin arguments, environment and stdout.
impl fmt::Display for CredentialPluginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Kubernetes credential plugin failed")
    }
}

impl fmt::Debug for CredentialPluginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialPluginError")
    }
}

impl Error for CredentialPluginError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source)
    }
}
