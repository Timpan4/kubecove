//! Client construction for each intent over the session's client caches
//! (ADR 0009). Finite clients belong to the rotating generation; confirmed
//! operations and live clients use their own non-rotating caches.

use super::ClientIntent;
use crate::commands::helpers::{client_cache::ClientGeneration, session_scope::SessionScope};
use crate::{
    commands::{
        helpers::{
            client_cache,
            client_cancellation::RequestCancellationLayer,
            forwarded_bearer::forwarded_bearer_layer,
            http_activity::{HTTP_ACTIVITY, HttpActivityLayer},
            read_stall::ReadStallLayer,
        },
        kubeconfig::{Fallback, KubeconfigSource},
    },
    models::{AppError, AppErrorKind},
};
use kube::{Client, client::ClientBuilder};
use std::{future::Future, time::Duration};
use tokio_util::sync::CancellationToken;

/// How long a request in flight may wait on a stalled server. The desktop
/// remote client also bounds its sign-in revocation with it.
pub const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(30);

enum ClientPath {
    Finite(ClientGeneration),
    Operation,
    Live,
}

impl ClientPath {
    fn cancellation(&self) -> Option<&CancellationToken> {
        match self {
            Self::Finite(generation) => Some(&generation.token),
            Self::Operation | Self::Live => None,
        }
    }

    fn lookup(&self, source: &str, context: &str, fingerprint: u64) -> Option<Client> {
        match self {
            Self::Finite(generation) => {
                client_cache::lookup_client(source, context, fingerprint, generation.id)
            }
            Self::Operation => client_cache::lookup_operation_client(source, context, fingerprint),
            Self::Live => client_cache::lookup_live_client(source, context, fingerprint),
        }
    }

    fn store(
        &self,
        source: String,
        context: String,
        fingerprint: u64,
        client: Client,
    ) -> Result<Client, AppError> {
        match self {
            Self::Finite(generation) => {
                if !client_cache::store_client(
                    source,
                    context,
                    fingerprint,
                    generation.id,
                    client.clone(),
                ) {
                    return Err(AppError::cancelled());
                }
            }
            Self::Operation => {
                client_cache::store_operation_client(source, context, fingerprint, client.clone());
            }
            Self::Live => {
                client_cache::store_live_client(source, context, fingerprint, client.clone());
            }
        }
        Ok(client)
    }
}

pub(super) async fn client(
    source: &KubeconfigSource,
    source_key: &str,
    intent: ClientIntent,
    cluster_context: &str,
) -> Result<Client, AppError> {
    let fallback = match intent {
        ClientIntent::Read | ClientIntent::Stream => Fallback::Allow,
        ClientIntent::Preview | ClientIntent::ConfirmedOperation | ClientIntent::LiveSession => {
            source.ensure_configured_sources_load()?;
            Fallback::Refuse
        }
    };
    let path = match intent {
        ClientIntent::Read | ClientIntent::Preview => {
            ClientPath::Finite(client_cache::finite_client_generation())
        }
        ClientIntent::ConfirmedOperation => ClientPath::Operation,
        ClientIntent::LiveSession | ClientIntent::Stream => ClientPath::Live,
    };
    let cancellation = path.cancellation().cloned();
    let build = Box::pin(async {
        let fingerprint = client_cache::fingerprint_files(&source.effective_kubeconfig_paths());
        if let Some(client) = path.lookup(source_key, cluster_context, fingerprint) {
            return Ok(client);
        }
        let gate = client_cache::client_build_gate(source_key, cluster_context);
        let guard = gate.lock_owned().await;
        if let Some(client) = path.lookup(source_key, cluster_context, fingerprint) {
            return Ok(client);
        }
        // Only the desktop plugin-path step below changes the config.
        #[cfg_attr(
            not(all(
                feature = "credential-plugin-path",
                any(target_os = "linux", target_os = "macos")
            )),
            allow(unused_mut)
        )]
        let mut config = source.config_for_context(cluster_context, fallback).await?;
        // Task locals do not propagate to blocking workers. Capture the bearer
        // layer and cache owner before crossing that boundary.
        let bearer = forwarded_bearer_layer()?;
        let scope = SessionScope::current_or_process();
        #[cfg(all(
            feature = "credential-plugin-path",
            any(target_os = "linux", target_os = "macos")
        ))]
        if matches!(scope.source_policy(), super::SourcePolicy::Local) {
            super::plugin_path::apply(&mut config).await;
        }
        let source_key = source_key.to_owned();
        let cluster_context = cluster_context.to_owned();
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            if path
                .cancellation()
                .is_some_and(CancellationToken::is_cancelled)
            {
                return Err(AppError::cancelled());
            }
            let builder = ClientBuilder::try_from(config).map_err(AppError::from)?;
            // The stall limit is per request, not a socket read timeout, so
            // idle pooled connections survive between bursts of reads.
            let stall = ReadStallLayer::new(REQUEST_READ_TIMEOUT);
            let client = match &path {
                // Only finite reads count toward load network time. Live watches
                // stay open for minutes and would make the busy clock meaningless.
                ClientPath::Finite(generation) => builder
                    .with_layer(&stall)
                    .with_layer(&HttpActivityLayer::new(&HTTP_ACTIVITY))
                    .with_layer(&RequestCancellationLayer::new(generation.token.clone()))
                    .with_layer(&bearer)
                    .build(),
                ClientPath::Operation => builder.with_layer(&stall).with_layer(&bearer).build(),
                ClientPath::Live => builder.with_layer(&bearer).build(),
            };
            scope.run_sync(|| path.store(source_key, cluster_context, fingerprint, client))
        })
        .await
        .map_err(|_| {
            AppError::new(
                "Kubernetes client construction failed.",
                AppErrorKind::Internal,
            )
        })?
    });
    match cancellation {
        Some(token) => resolve_before_cancel(build, &token).await,
        None => build.await,
    }
}

async fn resolve_before_cancel<T, F>(
    future: F,
    cancellation: &CancellationToken,
) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    tokio::select! {
        biased;
        () = cancellation.cancelled() => Err(AppError::cancelled()),
        result = future => result,
    }
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod exec_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AppErrorKind;

    #[tokio::test]
    async fn cancellation_interrupts_pending_config_resolution() {
        let cancellation = CancellationToken::new();
        let waiting = resolve_before_cancel(
            std::future::pending::<Result<(), AppError>>(),
            &cancellation,
        );
        cancellation.cancel();

        let error = waiting.await.expect_err("resolution should be cancelled");

        assert_eq!(error.kind, AppErrorKind::Cancelled);
    }
}
