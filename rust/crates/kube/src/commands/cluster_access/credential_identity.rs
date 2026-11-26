//! The local desktop's credential plugin identity (ADR 0037): enough for the
//! user to find and rerun the failing sign-in, without its arguments,
//! environment or output.

use crate::{
    commands::kubeconfig::{KubeconfigSource, default_kubeconfig_path},
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

pub(super) fn identity(
    source: &KubeconfigSource,
    cluster_context: &str,
    exec: &ExecConfig,
) -> CredentialIdentity {
    let user = source.read_kubeconfig().ok().and_then(|kubeconfig| {
        kubeconfig
            .contexts
            .into_iter()
            .find(|named| named.name == cluster_context)
            .and_then(|named| named.context?.user)
    });
    let kubeconfig = source
        .effective_kubeconfig_paths()
        .into_iter()
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    let reproduce = reproduce(
        cluster_context,
        &kubeconfig,
        default_kubeconfig_path().as_deref(),
        cfg!(windows),
    );
    CredentialIdentity {
        context: cluster_context.to_owned(),
        user,
        command: command(exec),
        kubeconfig: kubeconfig
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        reproduce,
    }
}

/// The program's file name and the bare subcommand words that follow it,
/// such as `kubectl oidc-login get-token`. The first flag, path, value or
/// other argument ends it, because later arguments can carry secrets.
fn command(exec: &ExecConfig) -> String {
    let program = exec.command.as_deref().map_or_default(|command| {
        Path::new(command).file_name().map_or_else(
            || command.to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    });
    exec.args
        .iter()
        .flatten()
        .take_while(|arg| is_subcommand_word(arg))
        .fold(program, |line, arg| format!("{line} {arg}"))
}

fn is_subcommand_word(arg: &str) -> bool {
    let mut chars = arg.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

/// `kubectl auth whoami` runs the context's sign-in and reports the signed-in
/// identity, and every authenticated user may call it.
fn reproduce(
    cluster_context: &str,
    kubeconfig: &[PathBuf],
    default: Option<&Path>,
    windows: bool,
) -> String {
    let command = format!(
        "kubectl --context {} auth whoami",
        quote(cluster_context, windows)
    );
    let uses_default = match kubeconfig {
        [] => true,
        [only] => Some(only.as_path()) == default,
        _ => false,
    };
    if uses_default {
        return command;
    }
    let Ok(joined) = std::env::join_paths(kubeconfig) else {
        return command;
    };
    let joined = quote(&joined.to_string_lossy(), windows);
    if windows {
        format!("$env:KUBECONFIG={joined}; {command}")
    } else {
        format!("KUBECONFIG={joined} {command}")
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
        format!("'{}'", value.replace('\'', "''"))
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
