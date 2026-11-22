//! Requests no fixture entry matches. They fail as errors instead of panicking: the
//! client often calls the backend inside a GPUI update, and a panic there stops effect
//! flushing for every later scenario in the process. The parity harness fails the
//! scenario at its end when any were recorded.
use super::Fixture;
use std::{fmt::Display, sync::Mutex};

#[derive(Default)]
pub struct Unexpected(Mutex<Vec<String>>);

impl Unexpected {
    /// Prints and records the request, and returns the message the client's error carries.
    pub fn record(&self, kind: &str, request: impl Display) -> String {
        let message = format!("unexpected {kind}: {request}");
        eprintln!("{message}");
        self.0.lock().unwrap().push(message.clone());
        message
    }
    pub fn requests(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl Fixture {
    /// Every request that matched no fixture entry in this scenario.
    // Only the parity harness checks them.
    #[allow(dead_code)]
    pub fn unexpected_requests(&self) -> Vec<String> {
        let mut requests = self.argo.unexpected.requests();
        requests.extend(self.coverage_state.unexpected.requests());
        requests
    }
}
