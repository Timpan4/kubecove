//! Server observations (ADR 0027). This is the only module that emits log
//! events, so the redaction rules live here: fields carry identifiers and
//! fixed reason codes, never credentials, error messages or Kubernetes data.
//! The one exception is [`server_failed`], whose startup errors are operator
//! configuration.

use crate::metrics::{
    self, AuthModeLabel, ErrorKindLabel, OutcomeLabel, ReasonLabel, RefusalLabels, SignInLabels,
};
use axum::http::{HeaderValue, Method, StatusCode};
use kubecove_lib::models::AppErrorKind;
use std::{fmt, time::Duration};
use tracing::{debug, error, info, warn, Level};
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

/// A startup or serve error that stops the server. Its message describes
/// operator configuration, such as a port or certificate path, and the
/// server printed it before structured logging existed. Without a
/// subscriber, as when arguments fail to parse, it goes to stderr as text.
pub fn server_failed(failure: &std::io::Error) {
    if tracing::dispatcher::has_been_set() {
        error!(event = "server_failed", error = %failure);
    } else {
        eprintln!("Error: {failure}");
    }
}

pub fn metrics_listening(address: std::net::SocketAddr) {
    info!(event = "metrics_listening", address = %address);
}

pub fn metrics_listener_failed(error: &std::io::Error) {
    warn!(event = "metrics_listener_failed", error_kind = %error.kind());
}

pub fn session_opened(session: SessionId, mode: AuthMode, subject: Option<&str>) {
    metrics::global()
        .sessions_opened
        .get_or_create(&AuthModeLabel {
            auth_mode: mode.as_str(),
        })
        .inc();
    info!(
        event = "session_opened",
        session_id = %session,
        auth_mode = mode.as_str(),
        subject = subject.map(|subject| tracing::field::display(escaped(subject))),
    );
}

pub fn session_resumed(session: SessionId, mode: AuthMode, subject: Option<&str>) {
    metrics::global()
        .sessions_resumed
        .get_or_create(&AuthModeLabel {
            auth_mode: mode.as_str(),
        })
        .inc();
    info!(
        event = "session_resumed",
        session_id = %session,
        auth_mode = mode.as_str(),
        subject = subject.map(|subject| tracing::field::display(escaped(subject))),
    );
}

pub fn session_closed(session: SessionId, reason: CloseReason) {
    metrics::global()
        .sessions_closed
        .get_or_create(&ReasonLabel {
            reason: reason.as_str(),
        })
        .inc();
    info!(event = "session_closed", session_id = %session, reason = reason.as_str());
}

pub fn session_expired(session: SessionId, grace: Duration) {
    metrics::global().sessions_expired.inc();
    info!(
        event = "session_expired",
        session_id = %session,
        grace_seconds = grace.as_secs(),
    );
}

pub fn session_cleanup_failed(session: SessionId, kind: AppErrorKind) {
    metrics::global()
        .session_cleanup_failures
        .get_or_create(&ErrorKindLabel {
            error_kind: kind.as_str(),
        })
        .inc();
    warn!(
        event = "session_cleanup_failed",
        session_id = %session,
        error_kind = kind.as_str(),
    );
}

pub fn sign_in_succeeded(mode: AuthMode, subject: Option<&str>) {
    metrics::global()
        .sign_ins
        .get_or_create(&SignInLabels {
            auth_mode: mode.as_str(),
            outcome: "success",
            reason: "none",
        })
        .inc();
    info!(
        event = "sign_in",
        auth_mode = mode.as_str(),
        outcome = "success",
        subject = subject.map(|subject| tracing::field::display(escaped(subject))),
    );
}

pub fn sign_in_failed(mode: AuthMode, reason: SignInFailure) {
    metrics::global()
        .sign_ins
        .get_or_create(&SignInLabels {
            auth_mode: mode.as_str(),
            outcome: "failure",
            reason: reason.as_str(),
        })
        .inc();
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
    metrics::global()
        .authentication_refused
        .get_or_create(&RefusalLabels {
            auth_mode: mode.as_str(),
            reason: reason.as_str(),
        })
        .inc();
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
    metrics::global()
        .requests_refused
        .get_or_create(&ReasonLabel {
            reason: reason.as_str(),
        })
        .inc();
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
    metrics::global()
        .oidc_refreshes
        .get_or_create(&OutcomeLabel {
            outcome: outcome.as_str(),
        })
        .inc();
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
    metrics::global().observe_command(command, outcome.as_str(), error_kind, duration);
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
#[path = "observe_capture.rs"]
pub(crate) mod capture;
