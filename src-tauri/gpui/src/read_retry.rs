//! Snapshot replacement recovery. Dropping the owner cancels the read and timer.
use crate::backend::Read;
use kubecove_lib::models::{AppError, AppErrorKind};
use std::future::Future;
use std::time::Duration;

pub(crate) async fn cancelled_read<T, Wait: Future<Output = ()>>(
    mut read: impl FnMut() -> Read<T>,
    mut wait: impl FnMut(Duration) -> Wait,
) -> Result<T, AppError> {
    // query-retry.ts permits two retries. TanStack 5.105.1 waits 1000 then 2000 ms.
    for delay in [1000, 2000] {
        match read().await {
            Err(error) if error.kind == AppErrorKind::Cancelled => {
                wait(Duration::from_millis(delay)).await;
            }
            result => return result,
        }
    }
    read().await
}
