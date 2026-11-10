//! The notice each desktop client shows for a selected remote that is not
//! connected (ADR 0031, "Session notices in the native UI"). The text is the
//! browser gate's, with "This browser's" changed to "This app's". The web
//! client keeps the same table in `src/lib/remote-notice.ts`, and the shared
//! parity scenarios hold the two to the same text.
use super::models::{RemoteAuthMode, RemoteConnectionSummary, RemoteSessionState};

/// An action a notice offers. Every notice keeps a way back to the local
/// backend: Disconnect, or Cancel while a session opens, so a client renders
/// the actions as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteNoticeAction {
    SignIn,
    CopySignInLink,
    /// Cancels the browser sign-in and keeps the server selected.
    CancelSignIn,
    StartNewSession,
    /// Cancels opening the session by returning to the local backend.
    CancelConnecting,
    Disconnect,
}

impl RemoteNoticeAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::SignIn => "Sign in",
            Self::CopySignInLink => "Copy sign-in link",
            Self::CancelSignIn | Self::CancelConnecting => "Cancel",
            Self::StartNewSession => "Start a new session",
            Self::Disconnect => "Disconnect",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteNotice {
    pub title: Option<&'static str>,
    pub message: &'static str,
    pub actions: &'static [RemoteNoticeAction],
}

pub const SIGN_IN_MEMORY_ONLY: &str = "This computer's credential store is unavailable, so the sign-in lasts only until KubeCove closes.";

/// Shown with `RemoteConnectionSummary::sign_in_not_deleted`, in place of a
/// sign-out that only looks complete.
pub const SIGN_IN_NOT_DELETED: &str = "This computer's credential store could not delete the sign-in, so it may still be stored. KubeCove will not use it while it runs. Remove any \"KubeCove remote sign-in\" entry for this server from the credential store.";

const SIGN_IN_AGAIN: &str = "Your sign-in has expired. Sign in again.";

impl RemoteConnectionSummary {
    /// The notice for this state, or `None` while the session is connected.
    pub fn notice(&self) -> Option<RemoteNotice> {
        use RemoteNoticeAction::{
            CancelConnecting, CancelSignIn, CopySignInLink, Disconnect, SignIn, StartNewSession,
        };
        let notice = |title, message, actions| {
            Some(RemoteNotice {
                title,
                message,
                actions,
            })
        };
        match self.session_state {
            RemoteSessionState::Connected => None,
            RemoteSessionState::SignInRequired => notice(
                Some("Sign in to KubeCove"),
                "Sign in through your browser to use this server.",
                &[SignIn, Disconnect],
            ),
            RemoteSessionState::Connecting => notice(
                Some("Connecting to KubeCove"),
                "Opening an authenticated session…",
                &[CancelConnecting],
            ),
            RemoteSessionState::WaitingForBrowser => notice(
                None,
                "Finish signing in in your browser.",
                &[CopySignInLink, CancelSignIn, Disconnect],
            ),
            RemoteSessionState::Reconnecting => notice(
                None,
                "Connection interrupted. Reconnecting to the current session.",
                &[Disconnect],
            ),
            RemoteSessionState::SessionEnded => notice(
                Some("Session ended"),
                "This app's server session closed or expired. Start a new session to continue. Other sessions on the server are not affected.",
                &[StartNewSession, Disconnect],
            ),
            // A token server that refuses its token, or whose grace window
            // elapsed, takes the token again in Settings.
            RemoteSessionState::SignInExpired | RemoteSessionState::SessionExpired => {
                match self.auth_mode {
                    RemoteAuthMode::Token => notice(
                        None,
                        "The server session expired. Reconnect with the server token.",
                        &[Disconnect],
                    ),
                    RemoteAuthMode::Oidc => notice(None, SIGN_IN_AGAIN, &[SignIn, Disconnect]),
                }
            }
        }
    }
}
