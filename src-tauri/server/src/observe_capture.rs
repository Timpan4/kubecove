//! Captures the production JSON output for one test thread.
//!
//! One global subscriber serves every test. A per-thread subscriber is
//! unreliable: while it is the only one registered, `tracing` caches a
//! callsite as disabled when another test thread reaches it first.
use super::{subscriber, LogFormat, DEFAULT_FILTER};
use std::{
    cell::RefCell,
    io,
    sync::{Arc, Mutex, Once},
};
use tracing::Dispatch;
use tracing_subscriber::{fmt::MakeWriter, EnvFilter};

thread_local! {
    static CURRENT: RefCell<Option<Captured>> = const { RefCell::new(None) };
}

#[derive(Clone, Default)]
pub struct Captured(Arc<Mutex<Vec<u8>>>);

/// Stops capturing on this thread when dropped.
pub struct Capturing;

impl Drop for Capturing {
    fn drop(&mut self) {
        CURRENT.with(|current| current.borrow_mut().take());
    }
}

impl Captured {
    /// Captures this thread's events at debug level and above. Tokio's
    /// current-thread test runtime polls every spawned task here.
    pub fn install(&self) -> Capturing {
        install_global();
        CURRENT.with(|current| *current.borrow_mut() = Some(self.clone()));
        Capturing
    }

    /// A production subscriber at the default filter that writes here.
    /// Scope a future to it with `WithSubscriber::with_subscriber`.
    pub fn at_default_level(&self) -> Dispatch {
        // With the global subscriber registered too, callsite interest
        // stays per-dispatch instead of being cached from this one.
        install_global();
        Dispatch::new(subscriber(
            LogFormat::Json,
            EnvFilter::new(DEFAULT_FILTER),
            self.clone(),
        ))
    }

    pub fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }

    /// Every line must be one JSON object.
    pub fn events(&self) -> Vec<serde_json::Value> {
        self.text()
            .lines()
            .map(|line| {
                let value: serde_json::Value = serde_json::from_str(line).unwrap();
                assert!(value.is_object(), "{line}");
                value
            })
            .collect()
    }

    pub fn named(&self, event: &str) -> Vec<serde_json::Value> {
        self.events()
            .into_iter()
            .filter(|value| value["event"] == event)
            .collect()
    }
}

fn install_global() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        tracing::subscriber::set_global_default(subscriber(
            LogFormat::Json,
            EnvFilter::new("kubecove_server=debug"),
            ThreadWriter,
        ))
        .expect("no other global test subscriber");
    });
}

impl io::Write for Captured {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl MakeWriter<'_> for Captured {
    type Writer = Self;

    fn make_writer(&self) -> Self {
        self.clone()
    }
}

/// Writes into the current thread's capture, if any.
struct ThreadWriter;

impl io::Write for ThreadWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        CURRENT.with(|current| {
            if let Some(captured) = current.borrow().as_ref() {
                captured.0.lock().unwrap().extend_from_slice(buffer);
            }
        });
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl MakeWriter<'_> for ThreadWriter {
    type Writer = Self;

    fn make_writer(&self) -> Self {
        Self
    }
}
