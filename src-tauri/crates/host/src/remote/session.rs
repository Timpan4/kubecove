//! OIDC sign-in, session restart and sign-out for the selected server
//! (ADR 0031). Token-mode connects stay in `remote_connect`.
use super::client::{self, RemoteConnection};
use super::http::{
    api_url, http_client, install_tls_provider, network_error, normalize_server_url,
    validation_error,
};
use super::models::{RemoteAuthMode, RemoteConnectionSummary, RemoteSessionState};
use super::sign_in::{self, NOT_COMPLETED, SignInFailure};
use super::store::{PendingSignIn, RemoteConnectionStore, Selection};
use kubecove_models::{AppError, AppErrorKind};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Selects a server that has no token in the request by the mode it reports.
pub(super) async fn select_server(
    server_url: &str,
    store: &RemoteConnectionStore,
) -> Result<RemoteConnectionSummary, AppError> {
    let server_url = normalize_server_url(server_url)?;
    install_tls_provider();
    let http = http_client()?;
    match sign_in::probe(&http, &server_url).await? {
        RemoteAuthMode::Token => Err(validation_error(
            "This server uses a server token. Enter the token to connect.",
        )),
        RemoteAuthMode::Oidc => {
            replace_selection(
                store,
                Selection::new(
                    server_url.clone(),
                    RemoteAuthMode::Oidc,
                    RemoteSessionState::SignInRequired,
                ),
                None,
            )
            .await;
            if let Some(credential) = store.sign_ins.get(&server_url) {
                open_session(store, &server_url, &credential, RemoteAuthMode::Oidc).await?;
            }
            Ok(selected_summary(store).await?)
        }
    }
}

/// Makes `selection` the selected server and closes whatever it replaces.
pub(super) async fn replace_selection(
    store: &RemoteConnectionStore,
    selection: Selection,
    connection: Option<Arc<RemoteConnection>>,
) {
    let previous = store
        .selection
        .lock()
        .expect("remote selection lock")
        .replace(selection);
    if let Some(pending) = previous.and_then(|previous| previous.pending) {
        pending.cancel();
    }
    let previous = std::mem::replace(&mut *store.current.write().await, connection);
    if let Some(previous) = previous {
        previous.close().await;
    }
}

/// Opens a session with `bearer`. A refused sign-in moves to "Sign-in
/// expired" and is forgotten.
async fn open_session(
    store: &RemoteConnectionStore,
    server_url: &str,
    bearer: &str,
    auth_mode: RemoteAuthMode,
) -> Result<(), AppError> {
    let mut before = RemoteSessionState::SignInRequired;
    store.update_selection(server_url, |selected| {
        before = selected.state;
        selected.state = RemoteSessionState::Connecting;
    });
    match client::connect(server_url, bearer, None, auth_mode).await {
        Ok(connection) => {
            if store.update_selection(server_url, |selected| {
                selected.state = RemoteSessionState::Connected;
            }) {
                let previous = store.current.write().await.replace(connection);
                if let Some(previous) = previous {
                    previous.close().await;
                }
            } else {
                // Disconnected while connecting.
                connection.close().await;
            }
            Ok(())
        }
        Err(failure) if failure.unauthorized => {
            if auth_mode == RemoteAuthMode::Oidc {
                store.sign_ins.forget(server_url);
            }
            store.update_selection(server_url, |selected| {
                selected.state = RemoteSessionState::SignInExpired;
                selected.memory_only = false;
            });
            drop(store.current.write().await.take());
            Ok(())
        }
        Err(failure) => {
            store.update_selection(server_url, |selected| selected.state = before);
            Err(failure.error)
        }
    }
}

async fn selected_summary(
    store: &RemoteConnectionStore,
) -> Result<RemoteConnectionSummary, AppError> {
    store.summary().await.ok_or_else(not_selected)
}

fn not_selected() -> AppError {
    AppError::new(
        "Select a remote KubeCove server first.",
        AppErrorKind::Session,
    )
}

fn oidc_selection(store: &RemoteConnectionStore) -> Result<Selection, AppError> {
    store
        .selection()
        .filter(|selected| selected.auth_mode == RemoteAuthMode::Oidc)
        .ok_or_else(|| {
            AppError::new(
                "Select a server that uses single sign-on first.",
                AppErrorKind::Session,
            )
        })
}

pub(super) async fn sign_in(
    store: &RemoteConnectionStore,
) -> Result<Option<RemoteConnectionSummary>, AppError> {
    let selected = oidc_selection(store)?;
    if selected.pending.is_some() {
        return Ok(store.summary().await);
    }
    let server_url = selected.server_url;
    let prepared = sign_in::prepare(&server_url, &sign_in::LOOPBACK).await?;
    let (cancel, cancelled) = oneshot::channel();
    let pending = Arc::new(PendingSignIn {
        link: prepared.link.clone(),
        cancel: Mutex::new(Some(cancel)),
    });
    if !store.update_selection(&server_url, |selected| {
        selected.state = RemoteSessionState::WaitingForBrowser;
        selected.pending = Some(Arc::clone(&pending));
    }) {
        return Ok(store.summary().await);
    }
    // A new sign-in replaces an ended session.
    if let Some(previous) = store.current.write().await.take() {
        previous.close().await;
    }
    // "Copy sign-in link" covers a browser that does not open.
    let _ = store.browser.open(&pending.link);
    install_tls_provider();
    let http = http_client()?;
    let result = prepared.finish(&http, &server_url, cancelled).await;
    store.update_selection(&server_url, |selected| {
        if selected
            .pending
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &pending))
        {
            selected.pending = None;
        }
    });
    let failure = match result {
        Ok(Ok(credential)) => {
            let memory_only = store.sign_ins.put(&server_url, credential.clone());
            store.update_selection(&server_url, |selected| selected.memory_only = memory_only);
            open_session(store, &server_url, &credential, RemoteAuthMode::Oidc).await?;
            return Ok(store.summary().await);
        }
        Ok(Err(SignInFailure::Cancelled)) => None,
        Ok(Err(SignInFailure::Denied)) => {
            Some(AppError::new(NOT_COMPLETED, AppErrorKind::Forbidden))
        }
        Err(error) => Some(error),
    };
    store.update_selection(&server_url, |selected| {
        selected.state = RemoteSessionState::SignInRequired;
    });
    match failure {
        Some(error) => Err(error),
        None => Ok(store.summary().await),
    }
}

pub(super) fn cancel_sign_in(store: &RemoteConnectionStore) {
    if let Some(pending) = store.selection().and_then(|selected| selected.pending) {
        pending.cancel();
    }
}

pub(super) fn sign_in_link(store: &RemoteConnectionStore) -> Option<String> {
    store
        .selection()
        .and_then(|selected| selected.pending)
        .map(|pending| pending.link.clone())
}

pub(super) async fn start_new_session(
    store: &RemoteConnectionStore,
) -> Result<RemoteConnectionSummary, AppError> {
    let selected = store.selection().ok_or_else(not_selected)?;
    let server_url = selected.server_url;
    let held = store
        .current
        .read()
        .await
        .as_ref()
        .map(|connection| connection.credentials.bearer_token.clone());
    let bearer = held.or_else(|| {
        (selected.auth_mode == RemoteAuthMode::Oidc)
            .then(|| store.sign_ins.get(&server_url))
            .flatten()
    });
    match bearer {
        Some(bearer) => open_session(store, &server_url, &bearer, selected.auth_mode).await?,
        None => {
            store.update_selection(&server_url, |selected| {
                selected.state = RemoteSessionState::SignInExpired;
            });
        }
    }
    selected_summary(store).await
}

/// Forgets the sign-in whether or not the server's revocation succeeds.
pub(super) async fn sign_out(
    store: &RemoteConnectionStore,
) -> Result<RemoteConnectionSummary, AppError> {
    let selected = oidc_selection(store)?;
    let server_url = selected.server_url;
    if let Some(pending) = selected.pending {
        pending.cancel();
    }
    let connection = store.current.write().await.take();
    match connection {
        Some(connection) => connection.sign_out().await,
        None => {
            if let Some(credential) = store.sign_ins.get(&server_url) {
                let _ = logout(&server_url, &credential).await;
            }
        }
    }
    store.sign_ins.forget(&server_url);
    store.update_selection(&server_url, |selected| {
        selected.state = RemoteSessionState::SignInRequired;
        selected.memory_only = false;
        selected.pending = None;
    });
    selected_summary(store).await
}

async fn logout(server_url: &str, credential: &str) -> Result<(), AppError> {
    install_tls_provider();
    http_client()?
        .post(api_url(server_url, "oidc/logout")?)
        .bearer_auth(credential)
        .send()
        .await
        .map_err(network_error)?;
    Ok(())
}
