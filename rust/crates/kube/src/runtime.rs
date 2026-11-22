//! Shared command state, message channels and session-scoped Tokio tasks.

use std::{fmt, future::Future, ops::Deref, sync::Arc};

/// A reference to host-managed command state.
pub struct State<'r, T: Send + Sync + 'static>(&'r T);

impl<'r, T: Send + Sync + 'static> State<'r, T> {
    pub fn new(value: &'r T) -> Self {
        Self(value)
    }

    pub fn inner(&self) -> &'r T {
        self.0
    }
}

impl<T: Send + Sync + 'static> Clone for State<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Send + Sync + 'static> Copy for State<'_, T> {}

impl<T: Send + Sync + 'static> Deref for State<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
    }
}

/// The receiving side of a channel is gone.
#[derive(Debug)]
pub struct ChannelClosed;

impl fmt::Display for ChannelClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("channel closed")
    }
}

impl std::error::Error for ChannelClosed {}

type Sink<T> = dyn Fn(T) -> Result<(), ChannelClosed> + Send + Sync;

/// Delivers typed messages to the host sink.
pub struct Channel<T> {
    sink: Arc<Sink<T>>,
}

impl<T> Channel<T> {
    pub fn new(sink: impl Fn(T) -> Result<(), ChannelClosed> + Send + Sync + 'static) -> Self {
        Self {
            sink: Arc::new(sink),
        }
    }

    pub fn send(&self, message: T) -> Result<(), ChannelClosed> {
        (self.sink)(message)
    }
}

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Self {
            sink: Arc::clone(&self.sink),
        }
    }
}

/// Handle to a session-scoped task.
pub struct JoinHandle<T>(tokio::task::JoinHandle<T>);

impl<T> JoinHandle<T> {
    pub fn abort(&self) {
        self.0.abort();
    }

    pub fn inner(&self) -> &tokio::task::JoinHandle<T> {
        &self.0
    }
}

/// Spawns on the host's Tokio runtime with the caller's session scope. Watches,
/// exec and port-forwards build Kubernetes clients inside it.
pub fn spawn<F>(task: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match crate::commands::SessionScope::current() {
        Some(scope) => JoinHandle(tokio::spawn(scope.run(task))),
        None => JoinHandle(tokio::spawn(task)),
    }
}

#[cfg(test)]
pub(crate) fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(future)
}
