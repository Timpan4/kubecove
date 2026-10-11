use super::*;
use axum::{
    extract::{Form, Query, State},
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct ProviderState {
    issuer: String,
    signing_key: Arc<EncodingKey>,
    jwk: Value,
    codes: Arc<Mutex<HashMap<String, Authorization>>>,
    claim_mode: Arc<Mutex<ClaimMode>>,
    exchanges: Arc<std::sync::atomic::AtomicUsize>,
    refresh_tokens: Arc<Mutex<HashMap<String, String>>>,
    refreshes: Arc<std::sync::atomic::AtomicUsize>,
    /// Added to `aud`, with `azp`, as an audience mapper does.
    extra_audience: Option<String>,
    refresh_gate: Arc<tokio::sync::Mutex<()>>,
    refresh_started: Arc<tokio::sync::Notify>,
}

impl ProviderState {
    fn audience(&self, claims: &mut Value) {
        if let Some(extra) = &self.extra_audience {
            claims["aud"] = json!([claims["aud"], extra]);
            claims["azp"] = json!("kubecove-test-client");
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ClaimMode {
    Valid,
    /// ID tokens expire two seconds after issue.
    ShortLived,
    RefreshRefused,
    RefreshUnavailable,
    /// Leaves the forwarding audience out of the login token.
    WithoutExtraAudience,
    WrongIssuer,
    WrongAudience,
    WrongSignature,
    Expired,
    NonceMismatch,
    WrongAuthorizedParty,
    MultiAudience,
    FutureIssuedAt,
    FutureNotBefore,
    UnsupportedAlgorithm,
    MissingIdToken,
    EmptySubject,
}

struct Authorization {
    redirect_uri: String,
    nonce: String,
    challenge: String,
    client_id: String,
}

pub(crate) struct MockProvider {
    pub base_url: String,
    shutdown: CancellationToken,
    task: JoinHandle<()>,
    claim_mode: Arc<Mutex<ClaimMode>>,
    exchanges: Arc<std::sync::atomic::AtomicUsize>,
    refreshes: Arc<std::sync::atomic::AtomicUsize>,
    pub(crate) refresh_gate: Arc<tokio::sync::Mutex<()>>,
    pub(crate) refresh_started: Arc<tokio::sync::Notify>,
}

impl MockProvider {
    pub(crate) fn set_claim_mode(&self, mode: ClaimMode) {
        *self.claim_mode.lock().unwrap() = mode;
    }

    pub(crate) fn exchanges(&self) -> usize {
        self.exchanges.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub(crate) fn refreshes(&self) -> usize {
        self.refreshes.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for MockProvider {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.task.abort();
    }
}

#[derive(Deserialize)]
struct AuthorizationRequest {
    redirect_uri: String,
    state: String,
    nonce: String,
    code_challenge: String,
    client_id: String,
}

#[derive(Deserialize)]
struct TokenRequest {
    code: String,
    redirect_uri: String,
    client_id: String,
    code_verifier: String,
}

pub(crate) async fn fixture(public_origin: &str) -> Result<(Oidc, MockProvider), String> {
    fixture_with(public_origin, None).await
}

/// `kubernetes_audience` turns on forwarding, and the provider then adds that
/// audience to every ID token.
pub(crate) async fn fixture_with(
    public_origin: &str,
    kubernetes_audience: Option<&str>,
) -> Result<(Oidc, MockProvider), String> {
    let origin = test_loopback_origin(public_origin)?;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| "mock OIDC listener failed")?;
    let address = listener
        .local_addr()
        .map_err(|_| "mock OIDC listener failed")?;
    let base_url = format!("http://{address}");
    let raw_key: Value = serde_json::from_str(include_str!("access_test_key.json"))
        .map_err(|_| "mock OIDC key fixture is invalid")?;
    let private_key = raw_key["privateDer"]
        .as_str()
        .ok_or("mock OIDC key fixture is invalid")?;
    let private_key = STANDARD
        .decode(private_key)
        .map_err(|_| "mock OIDC key fixture is invalid")?;
    let signing_key = EncodingKey::from_rsa_der(&private_key);
    let jwk = raw_key["jwk"].clone();
    let state = ProviderState {
        issuer: base_url.clone(),
        signing_key: Arc::new(signing_key),
        jwk,
        codes: Arc::new(Mutex::new(HashMap::new())),
        claim_mode: Arc::new(Mutex::new(ClaimMode::Valid)),
        exchanges: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        refresh_tokens: Arc::new(Mutex::new(HashMap::new())),
        refreshes: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        extra_audience: kubernetes_audience.map(str::to_owned),
        refresh_gate: Arc::default(),
        refresh_started: Arc::default(),
    };
    let claim_mode = Arc::clone(&state.claim_mode);
    let exchanges = Arc::clone(&state.exchanges);
    let refreshes = Arc::clone(&state.refreshes);
    let refresh_gate = Arc::clone(&state.refresh_gate);
    let refresh_started = Arc::clone(&state.refresh_started);
    let app = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/authorize", get(authorize))
        .route("/token", axum::routing::post(token))
        .route("/jwks", get(jwks))
        .with_state(state);
    let shutdown = CancellationToken::new();
    let signal = shutdown.clone();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(signal.cancelled_owned())
            .await;
    });

    let config = OidcConfig {
        issuer: base_url.clone(),
        client_id: "kubecove-test-client".into(),
        secret_file: None,
        login_timeout: Duration::from_secs(60),
        kubernetes_audience: kubernetes_audience.map(str::to_owned),
    };
    let http = client(config.login_timeout, false)?;
    let metadata = discover_metadata(&config.issuer, &http).await?;
    validate_metadata(&config.issuer, &metadata, true)?;
    let mut transaction_key = [0; 32];
    getrandom::fill(&mut transaction_key).map_err(|_| "mock OIDC transaction key failed")?;
    let oidc = Oidc {
        redirect_uri: format!("{origin}/api/v1/oidc/callback"),
        config,
        client_secret: None,
        http,
        metadata,
        transaction_key,
        credentials: Mutex::new(HashMap::new()),
        bound: Mutex::new(Vec::new()),
        allow_http_test: true,
    };
    Ok((
        oidc,
        MockProvider {
            base_url,
            shutdown,
            task,
            claim_mode,
            exchanges,
            refreshes,
            refresh_gate,
            refresh_started,
        },
    ))
}

pub(super) fn test_loopback_origin(origin: &str) -> Result<String, String> {
    let url = openidconnect::url::Url::parse(origin).map_err(|_| "invalid local test origin")?;
    if url.scheme() != "http"
        || url.username() != ""
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        })
    {
        return Err("test origin must be a loopback HTTP origin".into());
    }
    Ok(origin.trim_end_matches('/').to_owned())
}

async fn discovery(State(state): State<ProviderState>) -> Json<Value> {
    Json(json!({
        "issuer": state.issuer,
        "authorization_endpoint": format!("{}/authorize", state.issuer),
        "token_endpoint": format!("{}/token", state.issuer),
        "jwks_uri": format!("{}/jwks", state.issuer),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_methods_supported": ["none"]
    }))
}

async fn jwks(State(state): State<ProviderState>) -> Json<Value> {
    Json(json!({"keys": [state.jwk]}))
}

async fn authorize(
    State(provider): State<ProviderState>,
    Query(query): Query<AuthorizationRequest>,
) -> Result<Redirect, StatusCode> {
    let code = crate::security::random_token().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    provider
        .codes
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .insert(
            code.clone(),
            Authorization {
                redirect_uri: query.redirect_uri.clone(),
                nonce: query.nonce,
                challenge: query.code_challenge,
                client_id: query.client_id,
            },
        );
    let mut redirect =
        openidconnect::url::Url::parse(&query.redirect_uri).map_err(|_| StatusCode::BAD_REQUEST)?;
    redirect
        .query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &query.state);
    Ok(Redirect::to(redirect.as_str()))
}

async fn token(
    State(provider): State<ProviderState>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Response, StatusCode> {
    if form.get("grant_type").map(String::as_str) == Some("refresh_token") {
        provider.refresh_started.notify_one();
        let _refresh = provider.refresh_gate.lock().await;
        if matches!(
            *provider.claim_mode.lock().unwrap(),
            ClaimMode::RefreshRefused
        ) {
            return Ok((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_grant"})),
            )
                .into_response());
        }
        return refresh(&provider, &form).map(IntoResponse::into_response);
    }
    let token_request = TokenRequest {
        code: form.get("code").cloned().ok_or(StatusCode::BAD_REQUEST)?,
        redirect_uri: form
            .get("redirect_uri")
            .cloned()
            .ok_or(StatusCode::BAD_REQUEST)?,
        client_id: form
            .get("client_id")
            .cloned()
            .or_else(|| form.get("client_id").cloned())
            .unwrap_or_default(),
        code_verifier: form
            .get("code_verifier")
            .cloned()
            .ok_or(StatusCode::BAD_REQUEST)?,
    };
    let authorization = provider
        .codes
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .remove(&token_request.code)
        .ok_or(StatusCode::BAD_REQUEST)?;
    provider
        .exchanges
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let actual = PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(
        token_request.code_verifier,
    ));
    if token_request.redirect_uri != authorization.redirect_uri
        || token_request.client_id != authorization.client_id
        || actual.as_str() != authorization.challenge
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let now = unix_now();
    let mode = *provider
        .claim_mode
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut claims = json!({
        "iss": provider.issuer, "aud": authorization.client_id, "sub": "mock-user",
        "exp": now + 300, "iat": now, "nonce": authorization.nonce,
    });
    if !matches!(mode, ClaimMode::WithoutExtraAudience) {
        provider.audience(&mut claims);
    }
    match mode {
        ClaimMode::WrongIssuer => claims["iss"] = json!("https://attacker.example"),
        ClaimMode::WrongAudience => claims["aud"] = json!("other-client"),
        ClaimMode::Expired => claims["exp"] = json!(now.saturating_sub(1)),
        ClaimMode::NonceMismatch => claims["nonce"] = json!("wrong-nonce"),
        ClaimMode::EmptySubject => claims["sub"] = json!(""),
        ClaimMode::WrongAuthorizedParty => claims["azp"] = json!("other-client"),
        ClaimMode::MultiAudience => {
            claims["aud"] = json!(["kubecove-test-client", "untrusted-client"]);
            claims["azp"] = json!("kubecove-test-client");
        }
        ClaimMode::FutureIssuedAt => claims["iat"] = json!(now + 300),
        ClaimMode::FutureNotBefore => claims["nbf"] = json!(now + 300),
        ClaimMode::ShortLived => claims["exp"] = json!(now + 2),
        ClaimMode::Valid
        | ClaimMode::RefreshRefused
        | ClaimMode::RefreshUnavailable
        | ClaimMode::WithoutExtraAudience
        | ClaimMode::WrongSignature
        | ClaimMode::UnsupportedAlgorithm
        | ClaimMode::MissingIdToken => {}
    }
    let mut id_token = if matches!(mode, ClaimMode::UnsupportedAlgorithm) {
        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"test-only"),
        )
    } else {
        encode(
            &Header::new(Algorithm::RS256),
            &claims,
            &provider.signing_key,
        )
    }
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if matches!(mode, ClaimMode::WrongSignature) {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let mut segments = id_token.split('.');
        let header = segments.next().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        let payload = segments.next().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        let signature = segments.next().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        let mut signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        signature[0] ^= 1;
        id_token = format!("{header}.{payload}.{}", URL_SAFE_NO_PAD.encode(signature));
    }
    let refresh_token = issue_refresh_token(&provider, authorization.nonce)?;
    let response = if matches!(mode, ClaimMode::MissingIdToken) {
        json!({"access_token":"discarded-test-access-token","token_type":"Bearer","expires_in":300})
    } else {
        json!({"access_token":"discarded-test-access-token","token_type":"Bearer","expires_in":300,"id_token":id_token,"refresh_token":refresh_token})
    };
    Ok(Json(response).into_response())
}

fn issue_refresh_token(provider: &ProviderState, nonce: String) -> Result<String, StatusCode> {
    let token = crate::security::random_token().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    provider
        .refresh_tokens
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .insert(token.clone(), nonce);
    Ok(token)
}

// Rotates the refresh token, as Keycloak does, and keeps the login nonce.
fn refresh(
    provider: &ProviderState,
    form: &HashMap<String, String>,
) -> Result<Json<Value>, StatusCode> {
    let mode = *provider
        .claim_mode
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if matches!(mode, ClaimMode::RefreshUnavailable) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let nonce = provider
        .refresh_tokens
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .remove(form.get("refresh_token").ok_or(StatusCode::BAD_REQUEST)?)
        .ok_or(StatusCode::BAD_REQUEST)?;
    provider
        .refreshes
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let now = unix_now();
    let lifetime = if matches!(mode, ClaimMode::ShortLived) {
        2
    } else {
        300
    };
    let mut claims = json!({
        "iss": provider.issuer, "aud": "kubecove-test-client", "sub": "mock-user",
        "exp": now + lifetime, "iat": now, "nonce": nonce,
        "jti": crate::security::random_token().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    });
    provider.audience(&mut claims);
    if matches!(mode, ClaimMode::NonceMismatch) {
        claims["nonce"] = json!("wrong-nonce");
    }
    let id_token = encode(
        &Header::new(Algorithm::RS256),
        &claims,
        &provider.signing_key,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let refresh_token = issue_refresh_token(provider, nonce)?;
    let mut response = json!({
        "access_token": "discarded-test-access-token", "token_type": "Bearer",
        "expires_in": lifetime, "id_token": id_token, "refresh_token": refresh_token,
    });
    if matches!(mode, ClaimMode::MissingIdToken) {
        response.as_object_mut().unwrap().remove("id_token");
    }
    Ok(Json(response))
}
