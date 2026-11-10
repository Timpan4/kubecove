//! A local fake `KubeCove` server and browser for the sign-in tests. It speaks
//! just enough HTTP for the client and never reaches a real server, identity
//! provider, keyring or browser.
use super::sign_in::BrowserLauncher;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::{SinkExt, StreamExt};
use kubecove_gitops::commands::argo::SecureStore;
use kubecove_models::{AppError, AppErrorKind};
use reqwest::Url;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{Notify, mpsc},
};
use tokio_tungstenite::tungstenite::{
    Message,
    handshake::server::{ErrorResponse, Request, Response},
    http::StatusCode,
};

pub(super) const CREDENTIAL: &str = "issued-sign-in-credential-secret";
pub(super) const CODE: &str = "single-use-code-secret";

#[derive(Default)]
pub(super) struct FakeState {
    pub auth: Mutex<Value>,
    pub valid: Mutex<HashSet<String>>,
    /// The challenge and redirect URI the browser carried to the server.
    pub native: Mutex<Option<(String, String)>>,
    /// Verifiers presented at the token endpoint, for redaction checks.
    pub verifiers: Mutex<Vec<String>>,
    pub logouts: Mutex<Vec<String>>,
    pub closes: Mutex<usize>,
    pub grace: Mutex<u64>,
    /// Upgrades are refused with this status while set.
    pub refuse_upgrades: Mutex<Option<u16>>,
    pub stall_sessions: AtomicBool,
    pub session_requested: Notify,
    pub release_sessions: Notify,
    /// The token endpoint waits for `release_tokens` while set.
    pub stall_tokens: AtomicBool,
    pub token_requested: Notify,
    pub release_tokens: Notify,
    /// The logout endpoint never answers while set.
    pub stall_logouts: AtomicBool,
    pub logout_requested: Notify,
    /// The logout endpoint fails without revoking while set.
    pub refuse_logouts: AtomicBool,
    /// WebSocket connections the client has not closed yet.
    pub open_sockets: AtomicUsize,
    sockets: Mutex<Vec<mpsc::UnboundedSender<Option<String>>>>,
}

pub(super) struct FakeServer {
    pub url: String,
    pub state: Arc<FakeState>,
}

impl FakeServer {
    pub(super) async fn start(mode: Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(FakeState::default());
        *state.auth.lock().unwrap() = mode;
        *state.grace.lock().unwrap() = 30;
        let serving = Arc::clone(&state);
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(handle(stream, Arc::clone(&serving)));
            }
        });
        Self { url, state }
    }

    pub(super) async fn oidc() -> Self {
        Self::start(json!({"mode": "oidc", "authenticated": false, "nativeSignIn": true})).await
    }

    /// Sends a text frame to every open socket.
    pub(super) fn send(&self, frame: &str) {
        for socket in self.state.sockets.lock().unwrap().iter() {
            let _ = socket.send(Some(frame.to_owned()));
        }
    }

    /// Drops every open socket without a close frame.
    pub(super) fn drop_sockets(&self) {
        for socket in self.state.sockets.lock().unwrap().drain(..) {
            let _ = socket.send(None);
        }
    }

    pub(super) fn revoke(&self, credential: &str) {
        self.state.valid.lock().unwrap().remove(credential);
    }
}

async fn handle(stream: TcpStream, state: Arc<FakeState>) {
    const UPGRADE: &[u8] = b"GET /api/v1/channels";
    let mut start = [0_u8; UPGRADE.len()];
    // A peek can return part of the request line. Peek again until the bytes
    // decide the route, so a split upgrade request still reaches `websocket`.
    let read = loop {
        let read = stream.peek(&mut start).await.unwrap_or(0);
        if read == 0 || read == start.len() || !UPGRADE.starts_with(&start[..read]) {
            break read;
        }
        tokio::task::yield_now().await;
    };
    if start[..read] == *UPGRADE {
        websocket(stream, state).await;
        return;
    }
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut headers = HashMap::new();
    loop {
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        let Some((name, value)) = line.trim_end().split_once(':') else {
            break;
        };
        headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
    }
    let length = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await.unwrap();
    let bearer = headers
        .get("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_owned);
    let (status, body) = if path == "/api/v1/invoke/echo_credentials" {
        // A server whose error text quotes the request's credentials.
        let message = format!(
            "refused bearer {} session {}",
            bearer.unwrap_or_default(),
            headers.get("x-kubecove-session").map_or("", String::as_str)
        );
        refusal(500, &message, "internal")
    } else {
        route(&state, &method, &path, bearer, &body).await
    };
    let response = format!(
        "HTTP/1.1 {status} Fake\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = reader.into_inner().write_all(response.as_bytes()).await;
}

fn refusal(status: u16, message: &str, kind: &str) -> (u16, String) {
    (
        status,
        json!({"message": message, "kind": kind}).to_string(),
    )
}

async fn route(
    state: &FakeState,
    method: &str,
    path: &str,
    bearer: Option<String>,
    body: &[u8],
) -> (u16, String) {
    let signed_in = bearer
        .as_ref()
        .is_some_and(|bearer| state.valid.lock().unwrap().contains(bearer));
    match (method, path) {
        ("GET", "/api/v1/auth") => (200, state.auth.lock().unwrap().to_string()),
        ("POST", "/api/v1/oidc/native/token") => {
            if state.stall_tokens.load(Ordering::Acquire) {
                state.token_requested.notify_one();
                state.release_tokens.notified().await;
            }
            let request: Value = serde_json::from_slice(body).unwrap();
            let verifier = request["codeVerifier"].as_str().unwrap_or_default();
            state.verifiers.lock().unwrap().push(verifier.to_owned());
            let challenge = URL_SAFE_NO_PAD.encode(ring::digest::digest(
                &ring::digest::SHA256,
                verifier.as_bytes(),
            ));
            let expected = state.native.lock().unwrap().take();
            if request["code"] == CODE
                && expected.as_ref().is_some_and(|(stored, redirect)| {
                    *stored == challenge && request["redirectUri"] == redirect.as_str()
                })
            {
                state.valid.lock().unwrap().insert(CREDENTIAL.to_owned());
                (200, json!({"credential": CREDENTIAL}).to_string())
            } else {
                refusal(
                    400,
                    "desktop sign-in code is invalid or expired",
                    "forbidden",
                )
            }
        }
        ("POST", "/api/v1/session") if signed_in => {
            if state.stall_sessions.load(Ordering::Acquire) {
                state.session_requested.notify_one();
                state.release_sessions.notified().await;
            }
            let grace = *state.grace.lock().unwrap();
            (
                200,
                json!({"resumeToken": "resume-secret", "sessionGraceSeconds": grace, "serverVersion": "0.13.0"})
                    .to_string(),
            )
        }
        ("POST", "/api/v1/oidc/logout") => {
            if state.stall_logouts.load(Ordering::Acquire) {
                state.logout_requested.notify_one();
                std::future::pending::<()>().await;
            }
            if state.refuse_logouts.load(Ordering::Acquire) {
                return refusal(503, "logout is unavailable", "network");
            }
            if let Some(bearer) = bearer {
                state.valid.lock().unwrap().remove(&bearer);
                state.logouts.lock().unwrap().push(bearer);
            }
            (200, "true".into())
        }
        ("POST", "/api/v1/session/close") => {
            *state.closes.lock().unwrap() += 1;
            (200, "true".into())
        }
        ("POST", path) if path.starts_with("/api/v1/invoke/") && signed_in => (200, "true".into()),
        _ if !signed_in => refusal(401, "OIDC sign-in is required or has expired", "forbidden"),
        _ => refusal(404, "not found", "notFound"),
    }
}

// tungstenite's upgrade callback fixes the error type.
#[allow(clippy::result_large_err)]
async fn websocket(stream: TcpStream, state: Arc<FakeState>) {
    let check = Arc::clone(&state);
    let accepted = tokio_tungstenite::accept_hdr_async(
        stream,
        move |request: &Request, response: Response| {
            let bearer = request
                .headers()
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "));
            let refused = check.refuse_upgrades.lock().unwrap().or_else(|| {
                (!bearer.is_some_and(|bearer| check.valid.lock().unwrap().contains(bearer)))
                    .then_some(401)
            });
            match refused {
                Some(status) => {
                    let mut error = ErrorResponse::new(None);
                    *error.status_mut() = StatusCode::from_u16(status).unwrap();
                    Err(error)
                }
                None => Ok(response),
            }
        },
    )
    .await;
    let Ok(mut socket) = accepted else {
        return;
    };
    let (sender, mut commands) = mpsc::unbounded_channel();
    state.sockets.lock().unwrap().push(sender);
    state.open_sockets.fetch_add(1, Ordering::AcqRel);
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Some(frame)) => {
                    if socket.send(Message::Text(frame.into())).await.is_err() {
                        break;
                    }
                }
                _ => break,
            },
            incoming = socket.next() => if incoming.is_none_or(|message| message.is_err()) {
                break;
            },
        }
    }
    state.open_sockets.fetch_sub(1, Ordering::AcqRel);
}

/// What the fake browser does with a sign-in link.
#[derive(Clone, Copy)]
pub(super) enum Browser {
    /// Shows the provider's page and waits, as a user who has not finished.
    Idle,
    /// Completes the sign-in through the server.
    Complete,
    /// Sends the provider's denial.
    Deny,
    /// Sends a forged state and a wrong path first, then completes.
    ForgeThenComplete,
    /// Completes after the server lost the transaction, so it refuses the code.
    LostTransaction,
}

pub(super) struct FakeBrowser {
    pub server: Arc<FakeState>,
    pub behavior: Mutex<Browser>,
    pub opened: Mutex<Vec<String>>,
}

impl FakeBrowser {
    pub(super) fn new(server: &FakeServer, behavior: Browser) -> Arc<Self> {
        Arc::new(Self {
            server: Arc::clone(&server.state),
            behavior: Mutex::new(behavior),
            opened: Mutex::default(),
        })
    }
}

impl BrowserLauncher for FakeBrowser {
    fn open(&self, link: &str) -> Result<(), AppError> {
        self.opened.lock().unwrap().push(link.to_owned());
        let url = Url::parse(link).unwrap();
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        let redirect = query["redirect_uri"].clone();
        let behavior = *self.behavior.lock().unwrap();
        if !matches!(behavior, Browser::LostTransaction) {
            *self.server.native.lock().unwrap() =
                Some((query["code_challenge"].clone(), redirect.clone()));
        }
        let state = query["state"].clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let get = |url: String| client.get(url).send();
            match behavior {
                Browser::Idle => {}
                Browser::Complete | Browser::LostTransaction => {
                    let _ = get(format!("{redirect}?code={CODE}&state={state}")).await;
                }
                Browser::Deny => {
                    let _ = get(format!("{redirect}?error=access_denied&state={state}")).await;
                }
                Browser::ForgeThenComplete => {
                    let forged = get(format!("{redirect}?code={CODE}&state=forged"))
                        .await
                        .unwrap();
                    assert_eq!(forged.status(), 400);
                    let other = redirect.replace("/kubecove/callback", "/favicon.ico");
                    let missing = get(other).await.unwrap();
                    assert_eq!(missing.status(), 404);
                    let _ = get(format!("{redirect}?code={CODE}&state={state}")).await;
                }
            }
        });
        Ok(())
    }
}

/// Native credential storage in memory. `refuse` makes it unavailable;
/// `refuse_delete` makes only deletion fail.
#[derive(Default)]
pub(super) struct MemoryStore {
    pub values: Mutex<HashMap<(String, String), String>>,
    pub refuse: AtomicBool,
    pub refuse_delete: AtomicBool,
}

impl SecureStore for MemoryStore {
    fn read(&self, service: &str, id: &str) -> Result<Option<String>, AppError> {
        Ok(self
            .values
            .lock()
            .unwrap()
            .get(&(service.into(), id.into()))
            .cloned())
    }
    fn write(&self, service: &str, id: &str, value: &str) -> Result<(), AppError> {
        if self.refuse.load(Ordering::Acquire) {
            return Err(AppError::new(
                "native credential storage unavailable",
                AppErrorKind::CredentialUnavailable,
            ));
        }
        self.values
            .lock()
            .unwrap()
            .insert((service.into(), id.into()), value.into());
        Ok(())
    }
    fn delete(&self, service: &str, id: &str) -> Result<(), AppError> {
        if self.refuse_delete.load(Ordering::Acquire) {
            return Err(AppError::new(
                "native credential storage could not delete the entry",
                AppErrorKind::CredentialUnavailable,
            ));
        }
        self.values
            .lock()
            .unwrap()
            .remove(&(service.into(), id.into()));
        Ok(())
    }
}
