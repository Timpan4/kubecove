//! Finite-read policy shared with src/lib/query-retry.ts.
use crate::backend::Read;
use gpui_kit::BackgroundExecutor;
use kubecove_lib::models::{AppError, AppErrorKind};
use std::{sync::LazyLock, time::Duration};

fn transient(error: &AppError) -> bool {
    if matches!(
        error.kind,
        AppErrorKind::AdmissionDenied
            | AppErrorKind::CredentialUnavailable
            | AppErrorKind::FieldManagerConflict
            | AppErrorKind::Forbidden
            | AppErrorKind::ImmutableField
            | AppErrorKind::InvalidResource
            | AppErrorKind::Kubeconfig
            | AppErrorKind::LiveSessionTargetUnavailable
            | AppErrorKind::NotFound
            | AppErrorKind::ProviderDiscoveryUnavailable
            | AppErrorKind::Validation
            | AppErrorKind::Serialization
    ) {
        return false;
    }
    static DETERMINISTIC: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i-u)\bforbidden\b|\b403\b|\bnot ?found\b|\b404\b|admission webhook|fieldmanagerconflict|field is immutable|pod updates may not change fields|kubeconfig|failed to infer config")
            .expect("the web query retry pattern is valid")
    });
    !DETERMINISTIC.is_match(&error.message)
}

/// The caller's GPUI task owns attempts and timers. Dropping it cancels both.
pub(crate) fn read<T: Send + 'static>(
    executor: BackgroundExecutor,
    mut attempt: impl FnMut() -> Read<T> + Send + 'static,
) -> Read<T> {
    let first = attempt();
    Box::pin(retry_read(
        first,
        attempt,
        move |delay| executor.timer(delay),
        transient,
    ))
}

pub(crate) async fn cancelled_read<T, Wait: std::future::Future<Output = ()>>(
    mut read: impl FnMut() -> Read<T>,
    wait: impl FnMut(Duration) -> Wait,
) -> Result<T, AppError> {
    let first = read();
    retry_read(first, read, wait, |error| {
        error.kind == AppErrorKind::Cancelled && transient(error)
    })
    .await
}

async fn retry_read<T, Wait: std::future::Future<Output = ()>>(
    mut pending: Read<T>,
    mut read: impl FnMut() -> Read<T>,
    mut wait: impl FnMut(Duration) -> Wait,
    transient: impl Fn(&AppError) -> bool,
) -> Result<T, AppError> {
    // query-retry.ts permits two retries. TanStack 5.105.1 waits 1000 then 2000 ms.
    for delay in [1000, 2000] {
        match pending.await {
            Err(error) if transient(&error) => wait(Duration::from_millis(delay)).await,
            result => return result,
        }
        pending = read();
    }
    pending.await
}
