//! Client construction for each intent over the session's client caches
//! (ADR 0009). Finite clients belong to the rotating generation; confirmed
//! operations and live clients use their own non-rotating caches.

use super::ClientIntent;
use crate::{
    commands::{
        helpers::{
            client_cache, client_cancellation::RequestCancellationLayer,
            forwarded_bearer::forwarded_bearer_layer,
        },
        kubeconfig::{Fallback, KubeconfigSource},
    },
    models::AppError,
};
use kube::{client::ClientBuilder, Client, Config};
use std::{future::Future, time::Duration};
use tokio_util::sync::CancellationToken;

const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(30);

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
    let fingerprint = client_cache::fingerprint_files(&source.effective_kubeconfig_paths());
    match intent {
        ClientIntent::Read | ClientIntent::Preview => {
            finite_client(source, source_key, cluster_context, fingerprint, fallback).await
        }
        ClientIntent::ConfirmedOperation => {
            if let Some(client) =
                client_cache::lookup_operation_client(source_key, cluster_context, fingerprint)
            {
                return Ok(client);
            }
            let mut config = source.config_for_context(cluster_context, fallback).await?;
            config.read_timeout = Some(REQUEST_READ_TIMEOUT);
            let client = build_client(config)?;
            client_cache::store_operation_client(
                source_key.to_string(),
                cluster_context.to_string(),
                fingerprint,
                client.clone(),
            );
            Ok(client)
        }
        ClientIntent::LiveSession | ClientIntent::Stream => {
            if let Some(client) =
                client_cache::lookup_live_client(source_key, cluster_context, fingerprint)
            {
                return Ok(client);
            }
            let config = source.config_for_context(cluster_context, fallback).await?;
            let client = build_client(config)?;
            client_cache::store_live_client(
                source_key.to_string(),
                cluster_context.to_string(),
                fingerprint,
                client.clone(),
            );
            Ok(client)
        }
    }
}

async fn finite_client(
    source: &KubeconfigSource,
    source_key: &str,
    cluster_context: &str,
    fingerprint: u64,
    fallback: Fallback,
) -> Result<Client, AppError> {
    let generation = client_cache::finite_client_generation();
    if let Some(cached) =
        client_cache::lookup_client(source_key, cluster_context, fingerprint, generation.id)
    {
        return Ok(cached);
    }
    let mut config = resolve_before_cancel(
        source.config_for_context(cluster_context, fallback),
        &generation.token,
    )
    .await?;
    config.read_timeout = Some(REQUEST_READ_TIMEOUT);
    let bearer = forwarded_bearer_layer()?;
    let client = ClientBuilder::try_from(config)
        .map_err(AppError::from)?
        .with_layer(&RequestCancellationLayer::new(generation.token.clone()))
        .with_layer(&bearer)
        .build();
    if !client_cache::store_client(
        source_key.to_string(),
        cluster_context.to_string(),
        fingerprint,
        generation.id,
        client.clone(),
    ) {
        return Err(AppError::cancelled());
    }
    Ok(client)
}

fn build_client(config: Config) -> Result<Client, AppError> {
    let bearer = forwarded_bearer_layer()?;
    Ok(ClientBuilder::try_from(config)
        .map_err(AppError::from)?
        .with_layer(&bearer)
        .build())
}

async fn resolve_before_cancel<T, F>(
    future: F,
    cancellation: &CancellationToken,
) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    tokio::select! {
        result = future => result,
        () = cancellation.cancelled() => Err(AppError::cancelled()),
    }
}

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
