//! Server observations (ADR 0027). This is the only module that emits log
//! events, so the redaction rules live here: fields carry identifiers and
//! fixed reason codes, never credentials, error messages or Kubernetes data.

use axum::http::{HeaderValue, Method, StatusCode};
use kubecove_lib::models::AppErrorKind;
use std::{fmt, time::Duration};
use tracing::{debug, info, warn, Level};
use tracing_subscriber::{fmt::MakeWriter, EnvFilter};

/// Lifecycle events at info; dependency crates only report warnings.
pub const DEFAULT_FILTER: &str = "warn,kubecove_server=info";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

impl LogFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => Err("--log-format requires text or json".into()),
        }
    }
}

/// Reads `RUST_LOG`, or uses [`DEFAULT_FILTER`] when it is unset. An invalid
/// value fails startup instead of silently logging at another level.
pub fn filter_from_env() -> Result<EnvFilter, String> {
    match std::env::var("RUST_LOG") {
        Ok(directives) => EnvFilter::try_new(directives).map_err(|_| "RUST_LOG is invalid".into()),
        Err(std::env::VarError::NotPresent) => Ok(EnvFilter::new(DEFAULT_FILTER)),
        Err(std::env::VarError::NotUnicode(_)) => Err("RUST_LOG must be UTF-8".into()),
    }
}

/// The subscriber `main` installs; tests build it with a capturing writer.
pub fn subscriber<W>(
    format: LogFormat,
    filter: EnvFilter,
    writer: W,
) -> Box<dyn tracing::Subscriber + Send + Sync>
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer);
    match format {
        LogFormat::Text => Box::new(builder.finish()),
        LogFormat::Json => Box::new(
            builder
                .json()
                .flatten_event(true)
                .with_current_span(false)
                .with_span_list(false)
                .finish(),
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMode {
    Token,
    CloudflareAccess,
    Oidc,
}

impl AuthMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Token => "token",
            Self::CloudflareAccess => "cloudflare_access",
            Self::Oidc => "oidc",
        }
    }
}

/// A log identifier for one session. It is random and independent of the
/// resume token, so it reveals nothing about the session credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionId(u64);

impl SessionId {
    pub fn random() -> std::io::Result<Self> {
        let mut bytes = [0_u8; 8];
        getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
        Ok(Self(u64::from_le_bytes(bytes)))
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    Client,
    Logout,
    Shutdown,
    /// The sign-in could not be bound to the new session.
    ActivationFailed,
}

impl CloseReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Logout => "logout",
            Self::Shutdown => "shutdown",
            Self::ActivationFailed => "activation_failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignInFailure {
    InvalidToken,
    ProviderDenied,
    InvalidCallback,
    ProviderUnavailable,
}

impl SignInFailure {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidToken => "invalid_token",
            Self::ProviderDenied => "provider_denied",
            Self::InvalidCallback => "invalid_callback",
            Self::ProviderUnavailable => "provider_unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthRefusal {
    /// The request carried none of the mode's credential headers.
    Missing,
    Invalid,
    /// Signing keys or the identity provider could not be reached.
    Unavailable,
}

impl AuthRefusal {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestRefusal {
    Host,
    Origin,
}

impl RequestRefusal {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Origin => "origin",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshOutcome {
    Renewed,
    /// The provider refused and the ID token has expired; the sign-in ends.
    Refused,
    /// The provider answered with a token the server cannot accept.
    Unusable,
    /// The provider was unreachable or refused before the ID token expired.
    Failed,
}

impl RefreshOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Renewed => "renewed",
            Self::Refused => "refused",
            Self::Unusable => "unusable",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    Ok,
    /// The client or a newer request cancelled the command.
    Cancelled,
    Error(AppErrorKind),
    SessionEnded,
    AuthenticationEnded,
}

impl CommandOutcome {
    pub fn of_error(kind: AppErrorKind) -> Self {
        if kind == AppErrorKind::Cancelled {
            Self::Cancelled
        } else {
            Self::Error(kind)
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Cancelled => "cancelled",
            Self::Error(_) => "error",
            Self::SessionEnded => "session_ended",
            Self::AuthenticationEnded => "authentication_ended",
        }
    }
}

/// Subjects and header values come from identity providers and clients.
/// Escaping keeps control characters out of text-format log lines.
fn escaped(value: &str) -> impl fmt::Display + '_ {
    value.escape_debug()
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

pub fn server_started(mode: AuthMode, transport: &'static str) {
    info!(
        event = "server_started",
        version = env!("CARGO_PKG_VERSION"),
        auth_mode = mode.as_str(),
        transport,
    );
}

pub fn session_opened(session: SessionId, mode: AuthMode, subject: Option<&str>) {
    info!(
        event = "session_opened",
        session_id = %session,
        auth_mode = mode.as_str(),
        subject = subject.map(|subject| tracing::field::display(escaped(subject))),
    );
}

pub fn session_resumed(session: SessionId, mode: AuthMode, subject: Option<&str>) {
    info!(
        event = "session_resumed",
        session_id = %session,
        auth_mode = mode.as_str(),
        subject = subject.map(|subject| tracing::field::display(escaped(subject))),
    );
}

pub fn session_closed(session: SessionId, reason: CloseReason) {
    info!(event = "session_closed", session_id = %session, reason = reason.as_str());
}

pub fn session_expired(session: SessionId, grace: Duration) {
    info!(
        event = "session_expired",
        session_id = %session,
        grace_seconds = grace.as_secs(),
    );
}

pub fn session_cleanup_failed(session: SessionId, kind: AppErrorKind) {
    warn!(
        event = "session_cleanup_failed",
        session_id = %session,
        error_kind = kind.as_str(),
    );
}

pub fn sign_in_succeeded(mode: AuthMode, subject: Option<&str>) {
    info!(
        event = "sign_in",
        auth_mode = mode.as_str(),
        outcome = "success",
        subject = subject.map(|subject| tracing::field::display(escaped(subject))),
    );
}

pub fn sign_in_failed(mode: AuthMode, reason: SignInFailure) {
    warn!(
        event = "sign_in",
        auth_mode = mode.as_str(),
        outcome = "failure",
        reason = reason.as_str(),
    );
}

/// Maps the callback's error status: the provider redirected back with an
/// error, the provider or its keys were unreachable, or the callback failed
/// verification.
pub fn oidc_callback_failed(status: StatusCode) {
    sign_in_failed(
        AuthMode::Oidc,
        match status {
            StatusCode::SEE_OTHER => SignInFailure::ProviderDenied,
            StatusCode::SERVICE_UNAVAILABLE => SignInFailure::ProviderUnavailable,
            _ => SignInFailure::InvalidCallback,
        },
    );
}

pub fn authentication_refused(mode: AuthMode, reason: AuthRefusal) {
    info!(
        event = "authentication_refused",
        auth_mode = mode.as_str(),
        reason = reason.as_str(),
    );
}

/// Request headers are client-controlled and only bounded by the HTTP
/// parser, so their values are logged only when debug output is enabled.
pub fn request_refused(
    reason: RequestRefusal,
    method: &Method,
    host: Option<&HeaderValue>,
    origin: Option<&HeaderValue>,
) {
    let method = standard_method(method);
    if tracing::enabled!(Level::DEBUG) {
        let host = host.map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned());
        let origin = origin.map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned());
        warn!(
            event = "request_refused",
            reason = reason.as_str(),
            method,
            host = host
                .as_deref()
                .map(|host| tracing::field::display(escaped(host))),
            origin = origin
                .as_deref()
                .map(|origin| tracing::field::display(escaped(origin))),
        );
    } else {
        warn!(event = "request_refused", reason = reason.as_str(), method);
    }
}

/// Extension methods are arbitrary client text.
fn standard_method(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::DELETE => "DELETE",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        Method::CONNECT => "CONNECT",
        Method::PATCH => "PATCH",
        Method::TRACE => "TRACE",
        _ => "other",
    }
}

pub fn oidc_refresh(outcome: RefreshOutcome) {
    if outcome == RefreshOutcome::Renewed {
        debug!(event = "oidc_refresh", outcome = outcome.as_str());
    } else {
        warn!(event = "oidc_refresh", outcome = outcome.as_str());
    }
}

/// `command` is a declared command name or `unknown`, never request text.
pub fn command_completed(
    session: SessionId,
    command: &'static str,
    outcome: CommandOutcome,
    duration: Duration,
) {
    let error_kind = match outcome {
        CommandOutcome::Error(kind) => Some(kind.as_str()),
        _ => None,
    };
    if matches!(outcome, CommandOutcome::Ok | CommandOutcome::Cancelled) {
        debug!(
            event = "command_completed",
            session_id = %session,
            command,
            outcome = outcome.as_str(),
            duration_ms = duration_ms(duration),
        );
    } else {
        warn!(
            event = "command_completed",
            session_id = %session,
            command,
            outcome = outcome.as_str(),
            error_kind,
            duration_ms = duration_ms(duration),
        );
    }
}

#[cfg(test)]
pub(crate) mod capture {
    //! Captures the production JSON output for one test thread.
    //!
    //! One global subscriber serves every test. A per-thread subscriber is
    //! unreliable: while it is the only one registered, `tracing` caches a
    //! callsite as disabled when another test thread reaches it first.
    use super::{subscriber, LogFormat};
    use std::{
        cell::RefCell,
        io,
        sync::{Arc, Mutex, Once},
    };
    use tracing_subscriber::{fmt::MakeWriter, EnvFilter};

    thread_local! {
        static CURRENT: RefCell<Option<Captured>> = const { RefCell::new(None) };
    }

    #[derive(Clone, Default)]
    pub struct Captured(Arc<Mutex<Vec<u8>>>);

    /// Stops capturing on this thread when dropped.
    pub struct Capturing;

    impl Drop for Capturing {
        fn drop(&mut self) {
            CURRENT.with(|current| current.borrow_mut().take());
        }
    }

    impl Captured {
        /// Captures this thread's events at debug level and above. Tokio's
        /// current-thread test runtime polls every spawned task here.
        pub fn install(&self) -> Capturing {
            static INSTALL: Once = Once::new();
            INSTALL.call_once(|| {
                tracing::subscriber::set_global_default(subscriber(
                    LogFormat::Json,
                    EnvFilter::new("kubecove_server=debug"),
                    ThreadWriter,
                ))
                .expect("no other global test subscriber");
            });
            CURRENT.with(|current| *current.borrow_mut() = Some(self.clone()));
            Capturing
        }

        pub fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }

        /// Every line must be one JSON object.
        pub fn events(&self) -> Vec<serde_json::Value> {
            self.text()
                .lines()
                .map(|line| {
                    let value: serde_json::Value = serde_json::from_str(line).unwrap();
                    assert!(value.is_object(), "{line}");
                    value
                })
                .collect()
        }

        pub fn named(&self, event: &str) -> Vec<serde_json::Value> {
            self.events()
                .into_iter()
                .filter(|value| value["event"] == event)
                .collect()
        }
    }

    /// Writes into the current thread's capture, if any.
    struct ThreadWriter;

    impl io::Write for ThreadWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            CURRENT.with(|current| {
                if let Some(captured) = current.borrow().as_ref() {
                    captured.0.lock().unwrap().extend_from_slice(buffer);
                }
            });
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl MakeWriter<'_> for ThreadWriter {
        type Writer = Self;

        fn make_writer(&self) -> Self {
            Self
        }
    }
}
