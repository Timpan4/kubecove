pub mod access;
pub mod backend;
pub mod channels;
pub mod config;
pub mod dispatch;
pub mod files;
pub mod mcp;
pub mod metrics;
pub mod observe;
pub mod oidc;
pub mod security;
pub mod sessions;
pub mod tunnel;
mod websocket;

#[cfg(all(test, feature = "web-ui"))]
mod asset_tests;
#[cfg(feature = "web-ui")]
mod assets;

use access::{AccessIdentity, CloudflareAccess};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use kubecove_lib::models::{AppError, AppErrorKind};
use observe::{AuthMode, AuthRefusal, CloseReason, CommandOutcome, SignInFailure};
use oidc::names_cookie;
use security::{MissingOrigin, NetworkPolicy, authenticated, equal_secret};
use serde::Deserialize;
use serde_json::Value;
use sessions::Sessions;
use std::{sync::Arc, time::Instant};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct AuthenticationRevoked(CancellationToken);

pub struct AppState {
    pub token: String,
    pub policy: NetworkPolicy,
    pub sessions: Arc<Sessions>,
    pub credential_store: String,
    pub access: Option<Arc<CloudflareAccess>>,
    pub oidc: Option<Arc<oidc::Oidc>>,
    /// Set by `--mcp` (ADR 0028).
    pub mcp: Option<mcp::McpServer>,
}

impl AppState {
    pub fn auth_mode(&self) -> AuthMode {
        if self.oidc.is_some() {
            AuthMode::Oidc
        } else if self.access.is_some() {
            AuthMode::CloudflareAccess
        } else {
            AuthMode::Token
        }
    }
}

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub AppError);

impl ApiError {
    pub fn new(status: StatusCode, message: &str, kind: AppErrorKind) -> Self {
        Self(status, AppError::new(message, kind))
    }
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        let status = match error.kind {
            AppErrorKind::Session => StatusCode::CONFLICT,
            AppErrorKind::Validation => StatusCode::BAD_REQUEST,
            AppErrorKind::UnsupportedOperation => StatusCode::NOT_FOUND,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        Self(status, error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, [(header::CACHE_CONTROL, "no-store")], Json(self.1)).into_response()
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    let router = Router::new()
        .route("/api/v1/auth", get(authentication_mode))
        .route("/api/v1/oidc/login", post(oidc_login))
        .route("/api/v1/oidc/callback", get(oidc_callback))
        .route("/api/v1/oidc/native/start", get(oidc_native_start))
        .route("/api/v1/oidc/native/token", post(oidc_native_token))
        .route("/api/v1/oidc/logout", post(oidc_logout))
        .route("/api/v1/session", post(login))
        .route("/api/v1/session/close", post(close_session))
        .route("/api/v1/invoke/{command}", post(invoke))
        .route("/api/v1/channels", get(websocket::upgrade))
        .route("/api/v1/channels/{id}/close", post(close_channel));
    let router = if state.mcp.is_some() {
        router.route("/mcp", any(mcp::handle))
    } else {
        router
    };
    #[cfg(feature = "web-ui")]
    let router = router.fallback(get(assets::asset));
    router
        // Axum's default 2 MiB JSON limit matches the existing workspace import cap.
        .layer(middleware::from_fn_with_state(Arc::clone(&state), guard))
        .with_state(state)
}

async fn guard(
    State(state): State<Arc<AppState>>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    // HTTP/2 sends the host as `:authority`, which hyper puts in the URI,
    // and no Host header (RFC 9113, section 8.3.1).
    if !request.headers().contains_key(header::HOST)
        && let Some(host) = request
            .uri()
            .authority()
            .and_then(|authority| HeaderValue::from_str(authority.as_str()).ok())
    {
        request.headers_mut().insert(header::HOST, host);
    }
    let path = request.uri().path();
    let asset_request = path.starts_with("/assets/");
    // Agents send no Origin; a browser's Origin must still match. The MCP handler checks
    // its own bearer token, so a missing Origin never falls back to the server token.
    let mcp_request = path == "/mcp" && state.mcp.is_some();
    // The desktop's code exchange carries no cookie and no header credential;
    // the PKCE verifier in its body authenticates it (ADR 0031).
    let cookieless_code_exchange = state.oidc.is_some()
        && path == NATIVE_TOKEN_PATH
        && !request.headers().contains_key(header::ORIGIN)
        && !request.headers().contains_key(header::COOKIE);
    let needs_origin = if mcp_request {
        request.headers().contains_key(header::ORIGIN)
    } else if cookieless_code_exchange {
        false
    } else {
        request.method() == axum::http::Method::POST || path == "/api/v1/channels"
    };
    // Access stays as it is until its desktop sign-in is verified (ADR 0031).
    let missing_origin = if state.oidc.is_some() {
        MissingOrigin::HeaderCredential
    } else if state.access.is_some() {
        MissingOrigin::Refused
    } else {
        MissingOrigin::StartupToken(&state.token)
    };
    let checked = state
        .policy
        .check(request.headers(), needs_origin, missing_origin);
    if let Err(reason) = checked {
        observe::request_refused(
            reason,
            request.method(),
            request.headers().get(header::HOST),
            request.headers().get(header::ORIGIN),
        );
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "request Host or Origin is not allowed",
            AppErrorKind::Forbidden,
        )
        .into_response();
    }
    if let Some(oidc) = &state.oidc {
        if path.starts_with("/api/")
            && !matches!(
                path,
                "/api/v1/auth"
                    | "/api/v1/oidc/login"
                    | "/api/v1/oidc/callback"
                    | "/api/v1/oidc/native/start"
                    | NATIVE_TOKEN_PATH
            )
        {
            match oidc.verify(request.headers()) {
                Ok(session) => {
                    request.extensions_mut().insert(session.identity);
                    request
                        .extensions_mut()
                        .insert(AuthenticationRevoked(session.revoked));
                    request.extensions_mut().insert(session.credential);
                }
                Err(error) => {
                    let carried = names_cookie(request.headers(), oidc::AUTH_COOKIE)
                        || request.headers().contains_key(header::AUTHORIZATION);
                    observe::authentication_refused(AuthMode::Oidc, refusal(&error, carried));
                    return error.into_response();
                }
            }
        }
    } else if let Some(access) = &state.access {
        if path != "/api/v1/auth" {
            let identity = match access::assertion(request.headers()) {
                Ok(token) => access.verify(token).await,
                Err(error) => Err(error),
            };
            match identity {
                Ok(identity) => {
                    request.extensions_mut().insert(identity);
                }
                Err(error) => {
                    let carried = request.headers().contains_key(access::ASSERTION_HEADER);
                    observe::authentication_refused(
                        AuthMode::CloudflareAccess,
                        refusal(&error, carried),
                    );
                    return error.into_response();
                }
            }
        }
    } else if path.starts_with("/api/")
        && path != "/api/v1/session"
        && path != "/api/v1/auth"
        && !authenticated(request.headers(), &state.token)
    {
        let carried = request.headers().contains_key(header::AUTHORIZATION)
            || names_cookie(request.headers(), security::COOKIE_NAME);
        observe::authentication_refused(
            AuthMode::Token,
            if carried {
                AuthRefusal::Invalid
            } else {
                AuthRefusal::Missing
            },
        );
        return ApiError::new(
            StatusCode::UNAUTHORIZED,
            "authentication required",
            AppErrorKind::Forbidden,
        )
        .into_response();
    }
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .expect("validated Host");
    let policy = state.policy.content_security_policy(host);
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        policy.parse().expect("validated Host CSP"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        "nosniff".parse().expect("static header"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        "no-referrer".parse().expect("static header"),
    );
    if !asset_request || !headers.contains_key(header::CACHE_CONTROL) {
        headers.insert(
            header::CACHE_CONTROL,
            "no-store".parse().expect("static header"),
        );
    }
    response
}

async fn authentication_mode(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<Value> {
    if let Some(oidc) = &state.oidc {
        return Json(serde_json::json!({
            "mode": "oidc",
            "authenticated": oidc.verify(&headers).is_ok(),
            "nativeSignIn": true,
        }));
    }
    Json(
        serde_json::json!({"mode": if state.access.is_some() { "cloudflareAccess" } else { "token" }}),
    )
}

fn configured_oidc(state: &AppState) -> Result<&oidc::Oidc, ApiError> {
    state.oidc.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "OIDC is not configured",
            AppErrorKind::UnsupportedOperation,
        )
    })
}

async fn oidc_login(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    configured_oidc(&state)?.start()
}

async fn oidc_callback(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    callback: Result<Query<oidc::Callback>, QueryRejection>,
) -> Response {
    let response = async {
        let Query(callback) = callback.map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "OIDC callback is invalid",
                AppErrorKind::Validation,
            )
        })?;
        configured_oidc(&state)?.callback(&headers, callback).await
    }
    .await;
    let mut response = response.unwrap_or_else(IntoResponse::into_response);
    if !response.status().is_redirection() {
        response = (response.status(), axum::response::Html(SIGN_IN_FAILED_PAGE)).into_response();
    }
    response.headers_mut().append(
        header::SET_COOKIE,
        oidc::clear_cookie_header(oidc::TRANSACTION_COOKIE)
            .parse()
            .expect("static cookie"),
    );
    response
}

const NATIVE_TOKEN_PATH: &str = "/api/v1/oidc/native/token";
const SIGN_IN_FAILED_PAGE: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Sign-in failed</title><main><h1>Could not sign in</h1><p>The sign-in request was invalid, expired, or could not be verified.</p><a href=\"/\">Return to KubeCove and try again</a></main></html>";

/// The desktop's browser sign-in starts here (ADR 0031). It is a top-level
/// navigation, so failures render a page.
async fn oidc_native_start(
    State(state): State<Arc<AppState>>,
    query: Result<Query<oidc::NativeStart>, QueryRejection>,
) -> Response {
    let response = query
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "desktop sign-in request is invalid",
                AppErrorKind::Validation,
            )
        })
        .and_then(|Query(query)| configured_oidc(&state)?.native_start(query));
    match response {
        Ok(response) => response,
        Err(error) => (error.0, axum::response::Html(SIGN_IN_FAILED_PAGE)).into_response(),
    }
}

async fn oidc_native_token(
    State(state): State<Arc<AppState>>,
    payload: Result<Json<oidc::NativeToken>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(json_error)?;
    configured_oidc(&state)?.native_token(request)
}

async fn oidc_logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let oidc = configured_oidc(&state)?;
    let subject = oidc.logout(&headers)?;
    if let (Some(subject), Ok(resume)) = (subject, session_header(&headers)) {
        // A moved browser must not end a later owner's session, even for the same subject.
        if state
            .sessions
            .owner_as(resume, Some(&subject))
            .await
            .is_ok()
        {
            state.sessions.close(resume, CloseReason::Logout).await?;
        }
    }
    let mut response = Json(true).into_response();
    for cookie in oidc.clear_cookies()? {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    Ok(response)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Login {
    token: Option<String>,
    resume_token: Option<String>,
}

async fn login(
    State(state): State<Arc<AppState>>,
    identity: Option<Extension<AccessIdentity>>,
    credential: Option<Extension<oidc::OidcCredential>>,
    headers: HeaderMap,
    payload: Result<Json<Login>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(login) = payload.map_err(json_error)?;
    if state.access.is_none()
        && state.oidc.is_none()
        && !authenticated(&headers, &state.token)
        && !login
            .token
            .as_deref()
            .is_some_and(|token| equal_secret(token, &state.token))
    {
        observe::sign_in_failed(AuthMode::Token, SignInFailure::InvalidToken);
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid login token",
            AppErrorKind::Forbidden,
        ));
    }
    let session = state
        .sessions
        .authenticate_as(
            login.resume_token.as_deref(),
            access_subject(identity.as_ref()),
        )
        .await?;
    if state.auth_mode() == AuthMode::Token {
        observe::sign_in_succeeded(AuthMode::Token, None);
    }
    // Cleared when this session ends for any reason. A resumed session is
    // bound again; its sign-in may be a newer one from the same subject.
    if let (Some(oidc), Some(Extension(credential))) = (&state.oidc, credential)
        && oidc.forwards_credentials()
    {
        let opened = login.resume_token.is_none();
        let owner = state.sessions.owner(&session.resume_token).await?;
        let activation = async {
            oidc.ensure_fresh(&credential.0).await?;
            let _request = owner.request_gate.read().await;
            if owner.ended.is_cancelled() {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "session closed or expired",
                    AppErrorKind::Session,
                ));
            }
            oidc.bind(
                &credential,
                &owner.resume_token,
                Arc::clone(&owner.backend.scope),
                &owner.ended,
            )
        }
        .await;
        if let Err(error) = activation {
            // A session nobody can use must not hold a slot until it expires.
            if opened {
                let _ = state
                    .sessions
                    .close(&owner.resume_token, CloseReason::ActivationFailed)
                    .await;
            }
            return Err(error);
        }
        // The first login's task covers every later resume of the session.
        if opened {
            let oidc = Arc::clone(oidc);
            tokio::spawn(async move {
                owner.ended.cancelled().await;
                oidc.release_owner(&owner.resume_token);
            });
        }
    }
    let mut session = serde_json::to_value(session).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to serialize session",
            AppErrorKind::Internal,
        )
    })?;
    session["credentialStore"] = Value::String(state.credential_store.clone());
    let mut response = Json(session).into_response();
    if state.access.is_none() && state.oidc.is_none() {
        response.headers_mut().insert(
            header::SET_COOKIE,
            state
                .policy
                .cookie(&state.token)
                .parse()
                .expect("base64 cookie"),
        );
    }
    Ok(response)
}

async fn invoke(
    State(state): State<Arc<AppState>>,
    Path(command): Path<String>,
    identity: Option<Extension<AccessIdentity>>,
    revoked: Option<Extension<AuthenticationRevoked>>,
    headers: HeaderMap,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(args) = payload.map_err(json_error)?;
    let owner = state
        .sessions
        .owner_as(session_header(&headers)?, access_subject(identity.as_ref()))
        .await?;
    let _request = owner.request_gate.read().await;
    let started = Instant::now();
    let (outcome, result) = tokio::select! {
        biased;
        () = owner.ended.cancelled() => (
            CommandOutcome::SessionEnded,
            Err(ApiError::new(StatusCode::CONFLICT, "session opened elsewhere or expired", AppErrorKind::Session)),
        ),
        () = authentication_ended(identity.as_ref(), revoked.as_ref()) => (
            CommandOutcome::AuthenticationEnded,
            Err(ApiError::new(StatusCode::UNAUTHORIZED, "authentication expired or was revoked", AppErrorKind::Forbidden)),
        ),
        result = dispatch::dispatch(&owner.backend, &owner.channels, &command, &args) => match result {
            Ok(body) => (CommandOutcome::Ok, Ok(body)),
            Err(error) => (CommandOutcome::of_error(error.kind), Err(ApiError::from(error))),
        },
    };
    observe::command_completed(
        owner.session_id,
        dispatch::declared_command(&command),
        outcome,
        started.elapsed(),
    );
    result.map(|body| ([(header::CONTENT_TYPE, "application/json")], body).into_response())
}

/// Completes when the caller's access expires or its authentication is revoked; never when
/// neither applies.
pub(crate) async fn authentication_ended(
    identity: Option<&Extension<AccessIdentity>>,
    revoked: Option<&Extension<AuthenticationRevoked>>,
) {
    let expiry = async {
        match identity {
            Some(Extension(identity)) => tokio::time::sleep_until(identity.expires_at).await,
            None => std::future::pending().await,
        }
    };
    let cancellation = async {
        match revoked {
            Some(Extension(revoked)) => revoked.0.cancelled().await,
            None => std::future::pending().await,
        }
    };
    tokio::select! { () = expiry => {}, () = cancellation => {} }
}

async fn close_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
    identity: Option<Extension<AccessIdentity>>,
    headers: HeaderMap,
) -> Result<Json<bool>, ApiError> {
    let owner = state
        .sessions
        .owner_as(session_header(&headers)?, access_subject(identity.as_ref()))
        .await?;
    owner.channels.remove(id);
    Ok(Json(true))
}

async fn close_session(
    State(state): State<Arc<AppState>>,
    identity: Option<Extension<AccessIdentity>>,
    headers: HeaderMap,
) -> Result<Json<bool>, ApiError> {
    let resume = session_header(&headers)?;
    state
        .sessions
        .owner_as(resume, access_subject(identity.as_ref()))
        .await?;
    state.sessions.close(resume, CloseReason::Client).await?;
    Ok(Json(true))
}

fn refusal(error: &ApiError, carried: bool) -> AuthRefusal {
    if error.0 == StatusCode::SERVICE_UNAVAILABLE {
        AuthRefusal::Unavailable
    } else if carried {
        AuthRefusal::Invalid
    } else {
        AuthRefusal::Missing
    }
}

fn access_subject(identity: Option<&Extension<AccessIdentity>>) -> Option<&str> {
    identity.map(|Extension(identity)| identity.subject.as_str())
}

fn session_header(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get("x-kubecove-session")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::CONFLICT,
                "session credential required",
                AppErrorKind::Session,
            )
        })
}

// Match Result::map_err's owned rejection callback.
#[allow(clippy::needless_pass_by_value)]
fn json_error(error: JsonRejection) -> ApiError {
    ApiError::new(
        error.status(),
        "request body is invalid or exceeds the 2 MiB limit",
        AppErrorKind::Validation,
    )
}

#[cfg(any(feature = "web-ui", test))]
fn index_with_policy(html: &str, policy: &str) -> Option<String> {
    let marker = html.find("http-equiv=\"Content-Security-Policy\"")?;
    let start = html[..marker].rfind("<meta")?;
    let end = marker + html[marker..].find('>')? + 1;
    let mut html = html.to_owned();
    html.replace_range(
        start..end,
        &format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{policy}\">"),
    );
    Some(html)
}

#[cfg(test)]
mod api_tests;

#[cfg(test)]
mod oidc_router_tests;

#[cfg(test)]
mod mcp_tests;

#[cfg(test)]
mod oidc_native_router_tests;
