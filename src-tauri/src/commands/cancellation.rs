use crate::models::{AppError, CancelBackendRequestsResult};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex},
};
use tauri::State;
use tokio_util::sync::CancellationToken;

type RequestMap = BTreeMap<String, CancellationToken>;
type ScopeMap = BTreeMap<String, RequestMap>;

#[derive(Clone, Default)]
pub struct BackendCancellationRegistry {
    inner: Arc<Mutex<ScopeMap>>,
}

pub struct BackendCancellationGuard {
    registry: BackendCancellationRegistry,
    scope: Option<String>,
    request_id: Option<String>,
    token: CancellationToken,
}

fn normalized(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl BackendCancellationRegistry {
    pub fn register(
        &self,
        cancel_scope: Option<String>,
        request_id: Option<String>,
    ) -> BackendCancellationGuard {
        let scope = normalized(cancel_scope);
        let request_id = normalized(request_id);
        let token = CancellationToken::new();

        if let (Some(scope), Some(request_id)) = (&scope, &request_id) {
            let previous = self
                .inner
                .lock()
                .expect("backend cancellation registry mutex poisoned")
                .entry(scope.clone())
                .or_default()
                .insert(request_id.clone(), token.clone());
            if let Some(previous) = previous {
                previous.cancel();
            }
        }

        BackendCancellationGuard {
            registry: self.clone(),
            scope,
            request_id,
            token,
        }
    }

    pub fn cancel_scope(&self, cancel_scope: &str) -> usize {
        let tokens = self
            .inner
            .lock()
            .expect("backend cancellation registry mutex poisoned")
            .remove(cancel_scope)
            .unwrap_or_default()
            .into_values()
            .collect::<Vec<_>>();
        let count = tokens.len();
        for token in tokens {
            token.cancel();
        }
        count
    }

    fn unregister(&self, scope: &str, request_id: &str) {
        let mut guard = self
            .inner
            .lock()
            .expect("backend cancellation registry mutex poisoned");
        let Some(requests) = guard.get_mut(scope) else {
            return;
        };
        requests.remove(request_id);
        if requests.is_empty() {
            guard.remove(scope);
        }
    }
}

impl BackendCancellationGuard {
    pub async fn run<T>(
        &self,
        future: impl Future<Output = Result<T, AppError>>,
    ) -> Result<T, AppError> {
        tokio::select! {
            result = future => result,
            () = self.token.cancelled() => Err(AppError::cancelled()),
        }
    }
}

impl Drop for BackendCancellationGuard {
    fn drop(&mut self) {
        if let (Some(scope), Some(request_id)) = (&self.scope, &self.request_id) {
            self.registry.unregister(scope, request_id);
        }
    }
}

#[tauri::command]
pub fn cancel_backend_requests(
    cancel_scope: String,
    registry: State<'_, BackendCancellationRegistry>,
) -> CancelBackendRequestsResult {
    CancelBackendRequestsResult {
        cancelled: registry.cancel_scope(cancel_scope.trim()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_scope_only_cancels_matching_requests() {
        let registry = BackendCancellationRegistry::default();
        let first = registry.register(Some("scope-a".to_string()), Some("one".to_string()));
        let second = registry.register(Some("scope-a".to_string()), Some("two".to_string()));
        let third = registry.register(Some("scope-b".to_string()), Some("one".to_string()));

        assert_eq!(registry.cancel_scope("scope-a"), 2);
        assert!(first.token.is_cancelled());
        assert!(second.token.is_cancelled());
        assert!(!third.token.is_cancelled());
    }

    #[test]
    fn dropped_guard_unregisters_request() {
        let registry = BackendCancellationRegistry::default();
        let guard = registry.register(Some("scope-a".to_string()), Some("one".to_string()));
        drop(guard);

        assert_eq!(registry.cancel_scope("scope-a"), 0);
    }
}
