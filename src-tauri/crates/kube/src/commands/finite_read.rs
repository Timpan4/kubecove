//! Envelope for one finite read under ADR 0009. It owns optional request cancellation,
//! timing, backend log lines and the diagnostics record; the command supplies only the read.
//! Confirmed writes and live sessions do not use it.

use super::{
    diagnostics::{record_backend_cancelled, record_backend_error, record_backend_success},
    BackendCancellationRegistry,
};
use crate::models::{AppError, AppErrorKind, BackendDiagnosticField};
use std::{
    fmt::{Display, Write as _},
    future::Future,
    time::Instant,
};

struct Cancellation<'a> {
    registry: &'a BackendCancellationRegistry,
    cancel_scope: Option<String>,
    request_id: Option<String>,
}

pub struct FiniteRead<'a> {
    command: &'static str,
    labels: Vec<(&'static str, String)>,
    cancellation: Option<Cancellation<'a>>,
}

impl<'a> FiniteRead<'a> {
    pub fn new(command: &'static str) -> Self {
        Self {
            command,
            labels: Vec::new(),
            cancellation: None,
        }
    }

    /// Adds `key=value` to every log line of this read.
    #[must_use]
    pub fn label(mut self, key: &'static str, value: impl Display) -> Self {
        self.labels.push((key, value.to_string()));
        self
    }

    /// Registers the read so `cancel_backend_requests` can stop it by scope and request id.
    /// Workspace cancellation stops every finite read through its client generation.
    #[must_use]
    pub fn cancellable(
        mut self,
        registry: &'a BackendCancellationRegistry,
        cancel_scope: Option<String>,
        request_id: Option<String>,
    ) -> Self {
        self.cancellation = Some(Cancellation {
            registry,
            cancel_scope,
            request_id,
        });
        self
    }

    /// Runs `read` and records its outcome. `summary` describes a successful result for the
    /// log line and the diagnostics record.
    pub async fn run<T>(
        self,
        read: impl Future<Output = Result<T, AppError>>,
        summary: impl FnOnce(&T) -> Vec<BackendDiagnosticField>,
    ) -> Result<T, AppError> {
        let started = self.log_start();
        let result = match &self.cancellation {
            Some(cancellation) => {
                cancellation
                    .registry
                    .execute(
                        cancellation.cancel_scope.clone(),
                        cancellation.request_id.clone(),
                        read,
                    )
                    .await
            }
            None => read.await,
        };
        self.finish(started, &result, summary);
        result
    }

    /// Runs a synchronous read, such as a kubeconfig file read, and records its outcome.
    pub fn run_sync<T>(
        self,
        read: impl FnOnce() -> Result<T, AppError>,
        summary: impl FnOnce(&T) -> Vec<BackendDiagnosticField>,
    ) -> Result<T, AppError> {
        let started = self.log_start();
        let result = read();
        self.finish(started, &result, summary);
        result
    }

    fn log_start(&self) -> Instant {
        eprintln!("[kubecove:backend] {} start{}", self.command, self.labels());
        Instant::now()
    }

    fn labels(&self) -> String {
        self.labels
            .iter()
            .fold(String::new(), |mut line, (key, value)| {
                let _ = write!(line, " {key}={value}");
                line
            })
    }

    fn finish<T>(
        &self,
        started: Instant,
        result: &Result<T, AppError>,
        summary: impl FnOnce(&T) -> Vec<BackendDiagnosticField>,
    ) {
        let command = self.command;
        let labels = self.labels();
        let ms = started.elapsed().as_millis();
        match result {
            Ok(value) => {
                let fields = summary(value);
                // Summary fields already shown as labels are not repeated.
                let details = fields
                    .iter()
                    .filter(|field| !self.labels.iter().any(|(key, _)| *key == field.key))
                    .fold(String::new(), |mut line, field| {
                        let _ = write!(line, " {}={}", field.key, field.value);
                        line
                    });
                eprintln!("[kubecove:backend] {command} done{labels}{details} ms={ms}");
                record_backend_success(command, started, fields);
            }
            Err(error) if error.kind == AppErrorKind::Cancelled => {
                eprintln!("[kubecove:backend] {command} cancelled{labels} ms={ms}");
                record_backend_cancelled(command, started);
            }
            Err(error) => {
                eprintln!(
                    "[kubecove:backend] {command} error{labels} error_kind={} message={} ms={ms}",
                    error.kind, error.message
                );
                record_backend_error(command, started, error.kind.as_str());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{
        diagnostic_field, get_backend_diagnostics, set_backend_diagnostics_enabled, SessionScope,
    };
    use crate::models::BackendDiagnosticStatus;

    // Failure modes: a registered read that ignores its cancel scope, a cancellation recorded
    // as success or error, a registration left behind after completion, and success summary
    // fields recorded for a failed read.
    #[tokio::test]
    async fn records_each_outcome_and_stops_on_scope_cancellation() {
        SessionScope::new(None)
            .run(async {
                set_backend_diagnostics_enabled(true).unwrap();
                let registry = BackendCancellationRegistry::default();

                let rows = FiniteRead::new("read_ok")
                    .label("context", "kind-dev")
                    .cancellable(&registry, Some("scope".into()), Some("ok".into()))
                    .run(async { Ok::<_, AppError>(3) }, |rows| {
                        vec![diagnostic_field("rows", rows)]
                    })
                    .await;
                assert_eq!(rows.unwrap(), 3);
                assert_eq!(registry.cancel_scope("scope"), 0);

                let pending = FiniteRead::new("read_cancelled")
                    .cancellable(&registry, Some("scope".into()), Some("pending".into()))
                    .run(std::future::pending::<Result<(), AppError>>(), |()| {
                        Vec::new()
                    });
                let cancel = async {
                    tokio::task::yield_now().await;
                    assert_eq!(registry.cancel_scope("scope"), 1);
                };
                let (cancelled, ()) = tokio::join!(pending, cancel);
                assert_eq!(cancelled.unwrap_err().kind, AppErrorKind::Cancelled);

                let failed = FiniteRead::new("read_error")
                    .run(
                        async { Err::<(), _>(AppError::new("boom", AppErrorKind::Transport)) },
                        |()| vec![diagnostic_field("unused", 1)],
                    )
                    .await;
                assert_eq!(failed.unwrap_err().kind, AppErrorKind::Transport);

                let outcomes = get_backend_diagnostics()
                    .unwrap()
                    .into_iter()
                    .map(|event| (event.command, event.status, event.summary))
                    .collect::<Vec<_>>();
                assert_eq!(
                    outcomes,
                    vec![
                        (
                            "read_ok".to_string(),
                            BackendDiagnosticStatus::Ok,
                            vec![diagnostic_field("rows", 3)]
                        ),
                        (
                            "read_cancelled".to_string(),
                            BackendDiagnosticStatus::Cancelled,
                            Vec::new()
                        ),
                        (
                            "read_error".to_string(),
                            BackendDiagnosticStatus::Error,
                            vec![diagnostic_field("errorKind", "transport")]
                        ),
                    ]
                );
            })
            .await;
    }
}
