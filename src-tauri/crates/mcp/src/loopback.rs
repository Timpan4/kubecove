//! The desktop app's MCP listener on `127.0.0.1` (ADR 0028).

use crate::http::McpEndpoint;
use axum::{Router, extract::Request, response::IntoResponse as _};
use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Binds `127.0.0.1:port` and serves `/mcp` until `shutdown` is cancelled. A bind failure,
/// such as a port in use, is returned before anything is spawned. The returned task ends
/// once the listener is closed and in-flight requests have finished.
pub async fn serve_loopback(
    port: u16,
    endpoint: McpEndpoint,
    shutdown: CancellationToken,
) -> io::Result<(SocketAddr, JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
    let address = listener.local_addr()?;
    let router = Router::new().route(
        "/mcp",
        axum::routing::any(move |request: Request| {
            let endpoint = endpoint.clone();
            async move { endpoint.handle(request).await.into_response() }
        }),
    );
    let task = tokio::spawn(async move {
        // axum::serve retries failed accepts instead of ending the listener.
        let served = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await;
        if let Err(error) = served {
            eprintln!("[kubecove:mcp] loopback listener stopped: {error}");
        }
    });
    Ok((address, task))
}
