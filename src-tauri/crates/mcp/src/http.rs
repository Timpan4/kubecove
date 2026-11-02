//! Streamable HTTP endpoint for the MCP tools, in stateless JSON mode. The bearer token
//! is checked before the request reaches the MCP service (ADR 0028).

use crate::handler::{KubeCoveMcp, McpContext};
use bytes::Bytes;
use http::{HeaderMap, Request, Response, StatusCode, header};
use http_body_util::{BodyExt as _, Full, combinators::BoxBody};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
};
use std::{convert::Infallible, fmt::Display, sync::Arc};
use subtle::ConstantTimeEq as _;
use tokio_util::sync::CancellationToken;

pub type McpResponse = Response<BoxBody<Bytes, Infallible>>;

/// Host and Origin rules applied by the MCP service.
pub struct EndpointPolicy {
    /// Accepted `Host` values: a host name, or `host:port`.
    pub allowed_hosts: Vec<String>,
    /// Accepted `Origin` values. Empty rejects every request that carries an `Origin`.
    pub allowed_origins: Vec<String>,
}

impl EndpointPolicy {
    /// The desktop listener: loopback names only and no browser origins, so a web page
    /// cannot reach it by DNS rebinding or a cross-origin request.
    pub fn loopback() -> Self {
        Self {
            allowed_hosts: ["127.0.0.1", "localhost"].map(str::to_owned).to_vec(),
            allowed_origins: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct McpEndpoint {
    token: Arc<str>,
    shutdown: CancellationToken,
    service: StreamableHttpService<KubeCoveMcp, NeverSessionManager>,
}

impl McpEndpoint {
    /// Stops serving when `shutdown` is cancelled: in-flight tool calls are cancelled and
    /// later requests, including ones on connections still open, get 503.
    pub fn new(
        context: Arc<McpContext>,
        token: String,
        policy: EndpointPolicy,
        shutdown: CancellationToken,
    ) -> Self {
        let mut config = StreamableHttpServerConfig::default()
            .with_allowed_hosts(policy.allowed_hosts)
            .with_allowed_origins(policy.allowed_origins)
            .enforce_origin_validation()
            .with_sse_keep_alive(None);
        // Each request stands alone: no MCP session state is kept between requests.
        config.legacy_session_mode = false;
        config.json_response = true;
        config.cancellation_token = shutdown.clone();
        let handler = KubeCoveMcp::new(context);
        Self {
            token: token.into(),
            shutdown,
            service: StreamableHttpService::new(
                move || Ok(handler.clone()),
                Arc::new(NeverSessionManager::default()),
                config,
            ),
        }
    }

    /// Whether the request carries this endpoint's bearer token. Comparison is constant-time.
    pub fn authorized(&self, headers: &HeaderMap) -> bool {
        let mut values = headers.get_all(header::AUTHORIZATION).iter();
        let (Some(value), None) = (values.next(), values.next()) else {
            return false;
        };
        // The scheme name is case-insensitive (RFC 9110 section 11.1).
        value
            .to_str()
            .ok()
            .and_then(|value| value.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .is_some_and(|(_, presented)| {
                bool::from(presented.as_bytes().ct_eq(self.token.as_bytes()))
            })
    }

    /// Authenticates, then serves the MCP request.
    pub async fn handle<B>(&self, request: Request<B>) -> McpResponse
    where
        B: http_body::Body + Send + 'static,
        B::Data: Send + 'static,
        B::Error: Display,
    {
        if !self.authorized(request.headers()) {
            return unauthorized();
        }
        self.serve(request).await
    }

    /// Serves a request whose bearer token the caller has already checked.
    pub async fn serve<B>(&self, request: Request<B>) -> McpResponse
    where
        B: http_body::Body + Send + 'static,
        B::Data: Send + 'static,
        B::Error: Display,
    {
        use tower_service::Service as _;
        if self.shutdown.is_cancelled() {
            return json_error(StatusCode::SERVICE_UNAVAILABLE, "MCP server is stopped");
        }
        match self.service.clone().call(request).await {
            Ok(response) => response,
            Err(never) => match never {},
        }
    }
}

pub fn unauthorized() -> McpResponse {
    let mut response = json_error(StatusCode::UNAUTHORIZED, "MCP bearer token required");
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        http::HeaderValue::from_static("Bearer"),
    );
    response
}

fn json_error(status: StatusCode, message: &str) -> McpResponse {
    let body = serde_json::json!({ "error": message }).to_string();
    let mut response = Response::new(Full::new(Bytes::from(body)).boxed());
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    response
}
