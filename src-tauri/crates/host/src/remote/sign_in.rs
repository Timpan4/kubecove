//! The desktop half of the ADR 0031 OIDC sign-in: the server's mode probe, a
//! loopback listener, PKCE S256 and the code exchange. The verifier and state
//! never leave this module; the store keeps only the issued credential.
use super::http::{api_url, network_error, response_json, validation_error};
use super::models::RemoteAuthMode;
use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use kubecove_models::{AppError, AppErrorKind};
use reqwest::Client as HttpClient;
use serde::Deserialize;
use serde_json::json;
use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{Arc, Mutex},
};
use subtle::ConstantTimeEq;
use tokio::{net::TcpListener, sync::oneshot};

const CALLBACK_PATH: &str = "/kubecove/callback";
const FINISHED_PAGE: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Sign-in finished</title><main><h1>Sign-in finished</h1><p>You can close this browser tab and return to KubeCove.</p></main></html>";
const NOT_COMPLETED_PAGE: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Sign-in not completed</title><main><h1>Sign-in was not completed.</h1><p>Return to KubeCove to try again.</p></main></html>";
const UNVERIFIED_PAGE: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Sign-in not verified</title><main><h1>This sign-in could not be verified.</h1></main></html>";

pub(super) const NOT_COMPLETED: &str = "Sign-in was not completed.";
pub(super) const ACCESS_UNAVAILABLE: &str =
    "Desktop sign-in for Cloudflare Access is not available yet.";
pub(super) const NATIVE_SIGN_IN_UNSUPPORTED: &str =
    "This server does not support desktop sign-in. Update the server.";

/// Opens the system browser. Tests substitute a launcher that drives the
/// flow without a browser.
pub trait BrowserLauncher: Send + Sync {
    fn open(&self, url: &str) -> Result<(), AppError>;
}

pub struct SystemBrowser;

impl BrowserLauncher for SystemBrowser {
    fn open(&self, url: &str) -> Result<(), AppError> {
        // The URL never passes through `cmd`, where `&` separates commands.
        // On Windows the crate hands it to a fixed PowerShell script in an
        // environment variable; elsewhere it is one launcher argument (ADR
        // 0031). The source is dropped because an opener error may quote the URL.
        open::that_detached(url)
            .map_err(|_| AppError::new("Could not open the system browser", AppErrorKind::Io))
    }
}

/// The mode a server reports at `/api/v1/auth`, or why this desktop cannot
/// sign in to it.
pub(super) async fn probe(http: &HttpClient, server_url: &str) -> Result<RemoteAuthMode, AppError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Mode {
        mode: String,
        #[serde(default)]
        native_sign_in: bool,
    }
    let response = http
        .get(api_url(server_url, "auth")?)
        .send()
        .await
        .map_err(network_error)?;
    let mode: Mode = response_json(response).await?;
    match (mode.mode.as_str(), mode.native_sign_in) {
        ("token", _) => Ok(RemoteAuthMode::Token),
        ("oidc", true) => Ok(RemoteAuthMode::Oidc),
        ("oidc", false) => Err(AppError::new(
            NATIVE_SIGN_IN_UNSUPPORTED,
            AppErrorKind::UnsupportedOperation,
        )),
        ("cloudflareAccess", _) => Err(AppError::new(
            ACCESS_UNAVAILABLE,
            AppErrorKind::UnsupportedOperation,
        )),
        _ => Err(AppError::new(
            "The server reported an unknown sign-in mode.",
            AppErrorKind::UnsupportedOperation,
        )),
    }
}

pub(super) enum SignInFailure {
    Cancelled,
    Denied,
}

pub(super) struct PreparedSignIn {
    /// The browser link. It carries the challenge and state, not a secret.
    pub link: String,
    listener: TcpListener,
    redirect_uri: String,
    verifier: String,
    state: String,
}

/// IPv4 loopback first, then IPv6 (RFC 8252 7.3 and 8.3). Never `localhost`.
pub(super) const LOOPBACK: [(SocketAddr, &str); 2] = [
    (
        SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        "127.0.0.1",
    ),
    (
        SocketAddr::new(std::net::IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        "[::1]",
    ),
];

pub(super) async fn prepare(
    server_url: &str,
    candidates: &[(SocketAddr, &str)],
) -> Result<PreparedSignIn, AppError> {
    let mut bound = None;
    for (address, host) in candidates {
        if let Ok(listener) = TcpListener::bind(address).await {
            bound = Some((listener, *host));
            break;
        }
    }
    let (listener, host) = bound.ok_or_else(|| {
        AppError::new(
            "Could not listen on this computer's loopback address for the sign-in.",
            AppErrorKind::Io,
        )
    })?;
    let port = listener
        .local_addr()
        .map_err(|error| {
            AppError::new("Loopback listener failed", AppErrorKind::Io).with_source(error)
        })?
        .port();
    let redirect_uri = format!("http://{host}:{port}{CALLBACK_PATH}");
    let verifier = random_value()?;
    let state = random_value()?;
    let challenge = URL_SAFE_NO_PAD.encode(ring::digest::digest(
        &ring::digest::SHA256,
        verifier.as_bytes(),
    ));
    let mut link = api_url(server_url, "oidc/native/start")?;
    link.query_pairs_mut()
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state);
    Ok(PreparedSignIn {
        link: link.into(),
        listener,
        redirect_uri,
        verifier,
        state,
    })
}

/// 32 bytes from the OS CSPRNG, base64url without padding: a valid PKCE
/// verifier (RFC 7636 4.1) and the state the server accepts.
fn random_value() -> Result<String, AppError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| {
        AppError::new(
            "Secure random numbers are unavailable",
            AppErrorKind::Internal,
        )
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

struct Callback {
    expected_state: String,
    result: Mutex<Option<oneshot::Sender<Result<String, SignInFailure>>>>,
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn callback(
    State(callback): State<Arc<Callback>>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let state_matches = query.state.as_deref().is_some_and(|state| {
        bool::from(state.as_bytes().ct_eq(callback.expected_state.as_bytes()))
    });
    // A request without this sign-in's state is not the server's redirect;
    // keep waiting for that one.
    if !state_matches {
        return (StatusCode::BAD_REQUEST, Html(UNVERIFIED_PAGE)).into_response();
    }
    let outcome = match (query.code, query.error) {
        (Some(code), None) if !code.is_empty() => Ok(code),
        _ => Err(SignInFailure::Denied),
    };
    let page = if outcome.is_ok() {
        FINISHED_PAGE
    } else {
        NOT_COMPLETED_PAGE
    };
    if let Some(sender) = callback
        .result
        .lock()
        .expect("sign-in callback lock")
        .take()
    {
        let _ = sender.send(outcome);
    }
    Html(page).into_response()
}

impl PreparedSignIn {
    /// Waits for the browser's redirect until it arrives or `cancel` fires,
    /// then exchanges the code. The listener stops in either case. A cancel
    /// during the exchange returns at once; a credential the server issues
    /// after that is revoked, because nobody may hold it.
    pub(super) async fn finish(
        self,
        http: &HttpClient,
        server_url: &str,
        mut cancel: oneshot::Receiver<()>,
    ) -> Result<Result<String, SignInFailure>, AppError> {
        let (sender, receiver) = oneshot::channel();
        let app = Router::new()
            .route(CALLBACK_PATH, get(callback))
            .with_state(Arc::new(Callback {
                expected_state: self.state,
                result: Mutex::new(Some(sender)),
            }));
        let (stop, stopped) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(self.listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await;
        });
        let outcome = tokio::select! {
            outcome = receiver => outcome.unwrap_or(Err(SignInFailure::Cancelled)),
            _ = &mut cancel => Err(SignInFailure::Cancelled),
        };
        let _ = stop.send(());
        let code = match outcome {
            Ok(code) => code,
            Err(failure) => return Ok(Err(failure)),
        };
        let mut exchanging = Box::pin(exchange(
            http.clone(),
            server_url.to_owned(),
            code,
            self.verifier,
            self.redirect_uri,
        ));
        tokio::select! {
            issued = &mut exchanging => issued.map(Ok),
            _ = cancel => {
                let (http, server_url) = (http.clone(), server_url.to_owned());
                tokio::spawn(async move {
                    if let Ok(credential) = exchanging.await {
                        revoke(http, server_url, credential).await;
                    }
                });
                Ok(Err(SignInFailure::Cancelled))
            }
        }
    }
}

/// Revokes `credential` at the server. Failures are dropped: the caller has
/// already forgotten the credential, and the server ends it at its expiry.
/// The wait is bounded by the per-request stall limit the Kubernetes clients
/// use (`kubecove_kube::REQUEST_READ_TIMEOUT`), so a silent server cannot
/// hold the task.
pub(super) async fn revoke(http: HttpClient, server_url: String, credential: String) {
    let Ok(url) = api_url(&server_url, "oidc/logout") else {
        return;
    };
    let request = http.post(url).bearer_auth(&credential).send();
    let _ = tokio::time::timeout(kubecove_kube::REQUEST_READ_TIMEOUT, request).await;
}

async fn exchange(
    http: HttpClient,
    server_url: String,
    code: String,
    verifier: String,
    redirect_uri: String,
) -> Result<String, AppError> {
    #[derive(Deserialize)]
    struct Issued {
        credential: String,
    }
    let response = http
        .post(api_url(&server_url, "oidc/native/token")?)
        .json(&json!({"code": code, "codeVerifier": verifier, "redirectUri": redirect_uri}))
        .send()
        .await
        .map_err(network_error)?;
    let issued: Issued = response_json(response).await.map_err(|_| {
        AppError::new(
            "The server refused the sign-in. Sign in again.",
            AppErrorKind::Forbidden,
        )
    })?;
    if issued.credential.is_empty() {
        return Err(validation_error("The server returned an empty sign-in."));
    }
    Ok(issued.credential)
}

#[cfg(test)]
#[path = "sign_in_tests.rs"]
mod tests;
