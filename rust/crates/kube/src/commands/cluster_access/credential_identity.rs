//! The local desktop's credential plugin identity (ADR 0037): enough for the
//! user to find and rerun the failing sign-in, without its arguments,
//! environment or output.

use crate::{
    commands::kubeconfig::{KubeconfigOrigin, default_kubeconfig_path},
    models::{CredentialIdentity, CredentialPluginError, is_auth_error},
};
use http::Request;
use kube::config::ExecConfig;
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{BoxError, Layer, Service};

/// Subcommand words of common credential plugins. The spelling of an
/// argument cannot tell a subcommand from a positional value, so only these
/// exact leading sequences are shown; any other plugin shows its program name.
const KNOWN_SUBCOMMANDS: &[(&str, &[&str])] = &[
    ("kubectl", &["oidc-login", "get-token"]),
    ("kubelogin", &["get-token"]),
    ("kubectl-oidc_login", &["get-token"]),
    ("aws", &["eks", "get-token"]),
    ("aws-iam-authenticator", &["token"]),
    (
        "doctl",
        &["kubernetes", "cluster", "kubeconfig", "exec-credential"],
    ),
    ("oci", &["ce", "cluster", "generate-token"]),
    ("tsh", &["kube", "credentials"]),
    ("pinniped", &["login", "oidc"]),
    ("pinniped", &["login", "static"]),
    ("rancher", &["token"]),
];

pub(super) fn identity(
    origin: &KubeconfigOrigin,
    cluster_context: &str,
    exec: &ExecConfig,
) -> CredentialIdentity {
    CredentialIdentity {
        context: cluster_context.to_owned(),
        user: origin.user.clone(),
        command: command(exec),
        kubeconfig: origin
            .files
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        reproduce: reproduce(
            cluster_context,
            &origin.files,
            default_kubeconfig_path().as_deref(),
            cfg!(windows),
        ),
    }
}

/// The program's file name, followed by its subcommand words when the
/// plugin and its leading arguments match `KNOWN_SUBCOMMANDS`.
fn command(exec: &ExecConfig) -> String {
    let Some(command) = exec.command.as_deref() else {
        return String::new();
    };
    let program = Path::new(command).file_name().map_or_else(
        || command.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let stem = program.strip_suffix(".exe").unwrap_or(&program);
    let args = exec.args.as_deref().unwrap_or_default();
    KNOWN_SUBCOMMANDS
        .iter()
        .filter(|(name, words)| {
            *name == stem
                && words.len() <= args.len()
                && words
                    .iter()
                    .zip(args)
                    .all(|(word, arg)| arg.as_str() == *word)
        })
        .map(|(_, words)| *words)
        .max_by_key(|words| words.len())
        .map_or_else(
            || program.clone(),
            |words| format!("{program} {}", words.join(" ")),
        )
}

/// `kubectl auth whoami` runs the context's sign-in and reports the signed-in
/// identity, and every authenticated user may call it. `None` when the
/// kubeconfig files cannot be named exactly in a shell command.
fn reproduce(
    cluster_context: &str,
    kubeconfig: &[PathBuf],
    default: Option<&Path>,
    windows: bool,
) -> Option<String> {
    let context = quote(cluster_context, windows);
    match kubeconfig {
        [] => None,
        [only] if Some(only.as_path()) == default => {
            Some(format!("kubectl --context {context} auth whoami"))
        }
        [only] => Some(format!(
            "kubectl --kubeconfig {} --context {context} auth whoami",
            quote(only.to_str()?, windows)
        )),
        _ => {
            // `join_paths` fails when a name holds the path-list separator.
            let joined = std::env::join_paths(kubeconfig).ok()?;
            let joined = quote(joined.to_str()?, windows);
            Some(if windows {
                format!("$env:KUBECONFIG={joined}; kubectl --context {context} auth whoami")
            } else {
                format!("KUBECONFIG={joined} kubectl --context {context} auth whoami")
            })
        }
    }
}

/// Quotes for a POSIX shell, or for PowerShell on Windows.
fn quote(value: &str, windows: bool) -> String {
    // Characters that neither shell expands, splits or treats as operators.
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "._/-".contains(ch))
    {
        return value.to_owned();
    }
    if windows {
        // PowerShell also ends a single-quoted string at typographic single
        // quotes; doubling any of them keeps it literal.
        let mut quoted = String::with_capacity(value.len() + 2);
        quoted.push('\'');
        for ch in value.chars() {
            if matches!(ch, '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}') {
                quoted.push(ch);
            }
            quoted.push(ch);
        }
        quoted.push('\'');
        quoted
    } else {
        format!("'{}'", value.replace('\'', r"'\''"))
    }
}

/// Wraps request failures whose chain holds an authentication error, so a
/// token refresh that fails mid-session still names its plugin.
#[derive(Clone)]
pub(super) struct CredentialTagLayer {
    identity: Option<Arc<CredentialIdentity>>,
}

impl CredentialTagLayer {
    pub(super) const fn new(identity: Option<Arc<CredentialIdentity>>) -> Self {
        Self { identity }
    }
}

impl<S> Layer<S> for CredentialTagLayer {
    type Service = CredentialTagService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CredentialTagService {
            inner,
            identity: self.identity.clone(),
        }
    }
}

#[derive(Clone)]
pub(super) struct CredentialTagService<S> {
    inner: S,
    identity: Option<Arc<CredentialIdentity>>,
}

impl<S, B> Service<Request<B>> for CredentialTagService<S>
where
    S: Service<Request<B>>,
    S::Error: Into<BoxError>,
    S::Future: Send + 'static,
    S::Response: Send + 'static,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let identity = self.identity.clone();
        let future = self.inner.call(request);
        Box::pin(async move {
            future.await.map_err(|error| {
                let error: BoxError = error.into();
                match identity {
                    Some(identity) if is_auth_error(&*error) => {
                        Box::new(CredentialPluginError::new(identity, error))
                    }
                    _ => error,
                }
            })
        })
    }
}

#[cfg(test)]
#[path = "credential_identity_tests.rs"]
mod tests;
