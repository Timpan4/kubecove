mod client;
mod models;
mod tunnels;

pub use client::RemoteConnectionStore;
pub use models::RemoteConnectionSummary;

use client::connect;
use kubecove_kube::runtime::{Channel, State};
use kubecove_models::{AppError, AppErrorKind};
use serde_json::Value;
use std::sync::Arc;

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn remote_connect(
    server_url: String,
    token: String,
    store: State<'_, RemoteConnectionStore>,
) -> Result<RemoteConnectionSummary, AppError> {
    let connection = connect(&server_url, &token, None).await?;
    let summary = connection.summary.clone();
    let previous = store.current.write().await.replace(connection);
    if let Some(previous) = previous {
        previous.close().await;
    }
    Ok(summary)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn remote_disconnect(store: State<'_, RemoteConnectionStore>) -> Result<(), AppError> {
    let connection = store.current.write().await.take();
    if let Some(connection) = connection {
        connection.close().await;
    }
    Ok(())
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn remote_connection_status(
    store: State<'_, RemoteConnectionStore>,
) -> Result<Option<RemoteConnectionSummary>, AppError> {
    Ok(store.current.read().await.as_ref().map(|connection| {
        let mut summary = connection.summary.clone();
        summary.connected = connection.is_active();
        summary
    }))
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn remote_invoke(
    command: String,
    args: Value,
    store: State<'_, RemoteConnectionStore>,
) -> Result<Value, AppError> {
    let connection = current_connection(&store).await?;
    if let Some(value) = connection
        .tunnels
        .invoke(Arc::clone(&connection), &command, args.clone())
        .await?
    {
        return Ok(value);
    }
    connection.invoke(&command, args).await
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn remote_subscribe_channel(
    channel_id: u64,
    channel: Channel<Value>,
    store: State<'_, RemoteConnectionStore>,
) -> Result<(), AppError> {
    current_connection(&store)
        .await?
        .subscribe(channel_id, channel)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn remote_unsubscribe_channel(
    channel_id: u64,
    store: State<'_, RemoteConnectionStore>,
) -> Result<bool, AppError> {
    current_connection(&store)
        .await?
        .unsubscribe(channel_id)
        .await
}

async fn current_connection(
    store: &RemoteConnectionStore,
) -> Result<Arc<client::RemoteConnection>, AppError> {
    store
        .current
        .read()
        .await
        .clone()
        .filter(|connection| connection.is_active())
        .ok_or_else(|| {
            AppError::new(
                "connect to a remote KubeCove server first",
                AppErrorKind::Session,
            )
        })
}
