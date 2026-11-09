//! OIDC sign-in, session restart and sign-out for the selected server
//! (ADR 0031). Token-mode connects stay in `remote_connect`.
use super::client::{self, RemoteConnection};
use super::http::{http_client, install_tls_provider, normalize_server_url, validation_error};
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
                open_session(store, &server_url, &credential, RemoteAuthMode::Oidc, None).await?;
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
/// expired" and is forgotten. With `sign_in`, the session is kept only while
/// that sign-in has not ended.
async fn open_session(
    store: &RemoteConnectionStore,
    server_url: &str,
    bearer: &str,
    auth_mode: RemoteAuthMode,
    sign_in: Option<&PendingSignIn>,
) -> Result<(), AppError> {
    let mut before = RemoteSessionState::SignInRequired;
    store.update_selection(server_url, |selected| {
        before = selected.state;
        selected.state = RemoteSessionState::Connecting;
    });
    match client::connect(server_url, bearer, None, auth_mode).await {
        Ok(connection) => {
            // Sign out ends the sign-in before it takes `current`, so under
            // this lock the session is either published for Sign out to
            // close, or the ended sign-in is seen here.
            let mut current = store.current.write().await;
            let live = sign_in.is_none_or(|sign_in| !sign_in.is_cancelled());
            if live
                && store.update_selection(server_url, |selected| {
                    selected.state = RemoteSessionState::Connected;
                })
            {
                let previous = current.replace(connection);
                drop(current);
                if let Some(previous) = previous {
                    previous.close().await;
                }
            } else {
                // Disconnected or signed out while connecting.
                drop(current);
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
    install_tls_provider();
    let http = http_client()?;
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
    // The sign-in stays pending until the guard drops, so Sign out, Cancel and
    // Disconnect can end it while its session opens.
    let ended = EndSignIn {
        store,
        server_url: &server_url,
        pending: Arc::clone(&pending),
    };
    // A new sign-in replaces an ended session.
    if let Some(previous) = store.current.write().await.take() {
        previous.close().await;
    }
    // "Copy sign-in link" covers a browser that does not open.
    let _ = store.browser.open(&pending.link);
    let outcome = match prepared.finish(&http, &server_url, cancelled).await {
        Ok(Ok(credential)) => publish(store, &server_url, &pending, http, credential).await,
        Ok(Err(SignInFailure::Cancelled)) => Ok(()),
        Ok(Err(SignInFailure::Denied)) => {
            Err(AppError::new(NOT_COMPLETED, AppErrorKind::Forbidden))
        }
        Err(error) => Err(error),
    };
    drop(ended);
    outcome?;
    Ok(store.summary().await)
}

/// Clears a sign-in from the selection however `sign_in` stops: finished,
/// failed, or dropped while it awaits, as when the GPUI client replaces or
/// closes the task that runs it.
struct EndSignIn<'a> {
    store: &'a RemoteConnectionStore,
    server_url: &'a str,
    pending: Arc<PendingSignIn>,
}

impl Drop for EndSignIn<'_> {
    fn drop(&mut self) {
        self.store.update_selection(self.server_url, |selected| {
            let ours = selected
                .pending
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &self.pending));
            if ours {
                selected.pending = None;
            }
            // A sign-in that did not open a session leaves nothing to wait for.
            let stale = match selected.state {
                RemoteSessionState::WaitingForBrowser => selected.pending.is_none(),
                RemoteSessionState::Connecting => ours,
                _ => false,
            };
            if stale {
                selected.state = RemoteSessionState::SignInRequired;
            }
        });
    }
}

/// Keeps the issued credential and opens a session with it, unless the
/// sign-in ended first. Then nobody may hold the credential, so it is revoked.
async fn publish(
    store: &RemoteConnectionStore,
    server_url: &str,
    pending: &PendingSignIn,
    http: reqwest::Client,
    credential: String,
) -> Result<(), AppError> {
    let Some(memory_only) =
        pending.unless_cancelled(|| store.sign_ins.put(server_url, credential.clone()))
    else {
        tokio::spawn(sign_in::revoke(http, server_url.to_owned(), credential));
        return Ok(());
    };
    store.update_selection(server_url, |selected| selected.memory_only = memory_only);
    open_session(
        store,
        server_url,
        &credential,
        RemoteAuthMode::Oidc,
        Some(pending),
    )
    .await
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
        Some(bearer) => {
            open_session(store, &server_url, &bearer, selected.auth_mode, None).await?;
        }
        None => {
            store.update_selection(&server_url, |selected| {
                selected.state = RemoteSessionState::SignInExpired;
            });
        }
    }
    selected_summary(store).await
}

/// Forgets the sign-in here, then revokes it at the server. The order means a
/// server that never answers, or an app closed meanwhile, cannot keep it.
pub(super) async fn sign_out(
    store: &RemoteConnectionStore,
) -> Result<RemoteConnectionSummary, AppError> {
    let selected = oidc_selection(store)?;
    let server_url = selected.server_url;
    // Ending a pending sign-in first means its credential is either stored
    // before the `forget` below, or revoked by the sign-in itself.
    if let Some(pending) = selected.pending {
        pending.cancel();
    }
    let connection = store.current.write().await.take();
    let held = match &connection {
        Some(_) => None,
        None => store.sign_ins.get(&server_url),
    };
    store.sign_ins.forget(&server_url);
    store.update_selection(&server_url, |selected| {
        selected.state = RemoteSessionState::SignInRequired;
        selected.memory_only = false;
        selected.pending = None;
    });
    match (connection, held) {
        // Bounded by the session grace window.
        (Some(connection), _) => connection.sign_out().await,
        (None, Some(credential)) => {
            install_tls_provider();
            if let Ok(http) = http_client() {
                sign_in::revoke(http, server_url, credential).await;
            }
        }
        (None, None) => {}
    }
    selected_summary(store).await
}
