mod client;
mod http;
mod models;
mod notice;
mod session;
mod sign_in;
mod store;
mod tunnels;

#[cfg(test)]
#[path = "session_tests.rs"]
mod session_tests;
#[cfg(test)]
mod test_server;

pub use models::{RemoteAuthMode, RemoteConnectionSummary, RemoteSessionState};
pub use notice::{RemoteNotice, RemoteNoticeAction, SIGN_IN_MEMORY_ONLY};
pub use sign_in::BrowserLauncher;
pub use store::RemoteConnectionStore;

use kubecove_kube::runtime::{Channel, State};
use kubecove_models::{AppError, AppErrorKind};
use serde_json::{Value, value::RawValue};
use std::sync::Arc;

// The commands take Tauri state; the functions they call take the store, so
// tests can drive them without a Tauri app.

/// Connects with the server token, or, when `token` is empty, selects the
/// server by its sign-in mode: an OIDC server reuses a held sign-in or waits
/// for `remote_sign_in` (ADR 0031).
pub async fn remote_connect(
    server_url: String,
    token: String,
    store: State<'_, RemoteConnectionStore>,
) -> Result<RemoteConnectionSummary, AppError> {
    connect(server_url, token, &store).await
}

async fn connect(
    server_url: String,
    token: String,
    store: &RemoteConnectionStore,
) -> Result<RemoteConnectionSummary, AppError> {
    if token.trim().is_empty() {
        return session::select_server(&server_url, store).await;
    }
    let connection = client::connect(&server_url, &token, None, RemoteAuthMode::Token)
        .await
        .map_err(|failure| failure.error)?;
    let server_url = connection.summary.server_url.clone();
    session::replace_selection(
        store,
        store::Selection::new(
            server_url,
            RemoteAuthMode::Token,
            RemoteSessionState::Connected,
        ),
        Some(connection),
    )
    .await;
    Ok(store.summary().await.expect("connection selected above"))
}

/// Returns to the local backend. The sign-in stays held for the next connect.
pub async fn remote_disconnect(store: State<'_, RemoteConnectionStore>) -> Result<(), AppError> {
    disconnect(&store).await;
    Ok(())
}

async fn disconnect(store: &RemoteConnectionStore) {
    let selection = store
        .selection
        .lock()
        .expect("remote selection lock")
        .take();
    if let Some(pending) = selection.and_then(|selection| selection.pending) {
        pending.cancel();
    }
    let connection = store.current.write().await.take();
    if let Some(connection) = connection {
        connection.close().await;
    }
}

pub async fn remote_connection_status(
    store: State<'_, RemoteConnectionStore>,
) -> Result<Option<RemoteConnectionSummary>, AppError> {
    Ok(store.summary().await)
}

/// Signs in through the system browser and opens a session. It returns when
/// the session opens, the user cancels or disconnects, or the sign-in fails.
pub async fn remote_sign_in(
    store: State<'_, RemoteConnectionStore>,
) -> Result<Option<RemoteConnectionSummary>, AppError> {
    session::sign_in(&store).await
}

pub async fn remote_cancel_sign_in(
    store: State<'_, RemoteConnectionStore>,
) -> Result<(), AppError> {
    session::cancel_sign_in(&store);
    Ok(())
}

/// The pending sign-in's browser link, for "Copy sign-in link".
pub async fn remote_sign_in_link(
    store: State<'_, RemoteConnectionStore>,
) -> Result<Option<String>, AppError> {
    Ok(session::sign_in_link(&store))
}

/// Opens a new session without a resume token, with the credential this app
/// still holds, as the browser's "Start a new session" does.
pub async fn remote_start_new_session(
    store: State<'_, RemoteConnectionStore>,
) -> Result<RemoteConnectionSummary, AppError> {
    session::start_new_session(&store).await
}

/// Revokes the sign-in at the server and forgets it here. The server stays
/// selected.
pub async fn remote_sign_out(
    store: State<'_, RemoteConnectionStore>,
) -> Result<RemoteConnectionSummary, AppError> {
    session::sign_out(&store).await
}

pub async fn remote_invoke(
    command: String,
    args: Value,
    store: State<'_, RemoteConnectionStore>,
) -> Result<Value, AppError> {
    invoke(command, args, &store).await
}

async fn invoke(
    command: String,
    args: Value,
    store: &RemoteConnectionStore,
) -> Result<Value, AppError> {
    let result = invoke_remote(&current_connection(store).await?, &command, args).await;
    if result.is_err() {
        // Forgets a sign-in the server refused during this call.
        store.summary().await;
    }
    result
}

/// Runs `command` on the remote server, except `remote_intercepted` commands,
/// which the laptop's tunnel registry handles (ADR 0025).
async fn invoke_remote(
    connection: &Arc<client::RemoteConnection>,
    command: &str,
    args: Value,
) -> Result<Value, AppError> {
    if let Some(value) = connection
        .tunnels
        .invoke(connection, command, &args)
        .await?
    {
        return Ok(value);
    }
    connection.invoke(command, args).await
}

pub async fn remote_subscribe_channel(
    channel_id: u64,
    channel: Channel<Box<RawValue>>,
    store: State<'_, RemoteConnectionStore>,
) -> Result<(), AppError> {
    current_connection(&store)
        .await?
        .subscribe(channel_id, channel)
}

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
