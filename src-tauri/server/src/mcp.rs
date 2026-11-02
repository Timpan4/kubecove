//! `/mcp` for `kubecove-server --mcp` (ADR 0028). The router's guard has already applied
//! the Host and Origin policy, and Cloudflare Access when configured; this handler checks
//! the MCP bearer token and runs tools in one session scope with the operator's sources.

use crate::{
    ApiError, AppState,
    observe::{self, CommandOutcome, RequestRefusal, SessionId},
};
use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use kubecove_lib::{
    commands::{ClusterLiveStore, SessionScope},
    models::AppErrorKind,
};
use kubecove_mcp::{
    EndpointPolicy, McpContext, McpEndpoint, ToolCall, ToolOutcome, load_or_create_token,
};
use std::{io, path::Path, sync::Arc};
use tokio_util::sync::CancellationToken;

pub struct McpServer {
    endpoint: McpEndpoint,
}

impl McpServer {
    /// Loads or creates `mcp-token` in `data_dir`. `origins` are the browser origins the
    /// server already accepts.
    pub fn new(data_dir: &Path, origins: Vec<String>) -> io::Result<Self> {
        let token = load_or_create_token(data_dir)?;
        let session = SessionId::random()?;
        // Tools only read: they open no streams, exec sessions, forwards or Argo CD
        // connections, so this session needs a scope and caches, and no cleanup.
        let context = McpContext::new(ClusterLiveStore::default(), Some(SessionScope::new(None)))
            .with_observer(Arc::new(move |call: ToolCall| {
                let outcome = match call.outcome {
                    ToolOutcome::Ok => CommandOutcome::Ok,
                    ToolOutcome::Cancelled => CommandOutcome::Cancelled,
                    ToolOutcome::Failed(kind) => CommandOutcome::of_error(kind),
                };
                observe::command_completed(session, call.tool, outcome, call.elapsed);
            }));
        // The guard already checks Host against the server's policy, which accepts
        // non-loopback names under --allow-remote.
        let policy = EndpointPolicy {
            allowed_hosts: Vec::new(),
            allowed_origins: origins,
        };
        Ok(Self {
            endpoint: McpEndpoint::new(Arc::new(context), token, policy, CancellationToken::new()),
        })
    }
}

pub async fn handle(State(state): State<Arc<AppState>>, request: Request) -> Response {
    let Some(mcp) = &state.mcp else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !mcp.endpoint.authorized(request.headers()) {
        observe::request_refused(
            RequestRefusal::McpToken,
            request.method(),
            request.headers().get(header::HOST),
            request.headers().get(header::ORIGIN),
        );
        let mut response = ApiError::new(
            StatusCode::UNAUTHORIZED,
            "MCP bearer token required",
            AppErrorKind::Forbidden,
        )
        .into_response();
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Bearer"),
        );
        return response;
    }
    mcp.endpoint.serve(request).await.into_response()
}
