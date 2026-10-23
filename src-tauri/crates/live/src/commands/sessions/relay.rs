//! The accept-and-relay loop shared by port-forward listeners and the private
//! Argo CD Service tunnel.

use crate::commands::SessionScope;
use crate::models::AppError;
use crate::runtime::JoinHandle;
use std::{
    future::Future,
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    task::{JoinError, JoinSet},
};

const MAX_CONSECUTIVE_ACCEPT_FAILURES: u32 = 5;
const ACCEPT_RETRY_DELAY: Duration = Duration::from_millis(200);

/// What happened on a relay, for its owner to report.
pub enum RelayEvent {
    /// Accepting failed; the relay tries again after a short delay.
    AcceptRetry {
        attempt: u32,
        error: io::Error,
    },
    /// Accepting failed too often in a row; the relay has stopped.
    AcceptFailed(io::Error),
    /// A connection finished. `idle` is true when no other connection is open.
    ConnectionClosed {
        idle: bool,
    },
    ConnectionFailed(AppError),
    ConnectionPanicked(JoinError),
}

/// Binds a listener on `127.0.0.1`. `None` lets the OS pick a free port.
pub async fn bind_loopback(port: Option<u16>) -> io::Result<(TcpListener, u16)> {
    let listener =
        TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port.unwrap_or(0))).await?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

/// Accepts connections on `listener` and relays each one through
/// `relay_connection` in its own task, until the returned handle is aborted.
/// Aborting it closes the listener and every open connection.
///
/// Every connection runs in the caller's session scope, so a connection that
/// builds a Kubernetes client builds it for the caller's session.
pub fn spawn_relay<C, F, E>(
    listener: TcpListener,
    relay_connection: C,
    mut on_event: E,
) -> JoinHandle<()>
where
    C: Fn(TcpStream) -> F + Send + 'static,
    F: Future<Output = Result<(), AppError>> + Send + 'static,
    E: FnMut(RelayEvent) + Send + 'static,
{
    let scope = SessionScope::current();
    crate::runtime::spawn(async move {
        let mut connections = JoinSet::new();
        let mut consecutive_accept_failures = 0;
        loop {
            tokio::select! {
                accepted = listener.accept() => match accepted {
                    Ok((stream, _)) => {
                        consecutive_accept_failures = 0;
                        connections.spawn(in_scope(scope.clone(), relay_connection(stream)));
                    }
                    Err(error) => {
                        consecutive_accept_failures += 1;
                        if consecutive_accept_failures >= MAX_CONSECUTIVE_ACCEPT_FAILURES {
                            on_event(RelayEvent::AcceptFailed(error));
                            break;
                        }
                        on_event(RelayEvent::AcceptRetry { attempt: consecutive_accept_failures, error });
                        tokio::time::sleep(ACCEPT_RETRY_DELAY).await;
                    }
                },
                Some(finished) = connections.join_next(), if !connections.is_empty() => {
                    on_event(match finished {
                        Ok(Ok(())) => RelayEvent::ConnectionClosed { idle: connections.is_empty() },
                        Ok(Err(error)) => RelayEvent::ConnectionFailed(error),
                        Err(error) => RelayEvent::ConnectionPanicked(error),
                    });
                }
            }
        }
    })
}

async fn in_scope<F: Future>(scope: Option<Arc<SessionScope>>, future: F) -> F::Output {
    match scope {
        Some(scope) => scope.run(future).await,
        None => future.await,
    }
}
