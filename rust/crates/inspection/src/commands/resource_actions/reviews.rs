use super::state::{action_review_value, action_value};
use crate::models::{AppError, AppErrorKind, ResourceActionConfirmation, ResourceActionRequest};
use kube::api::DynamicObject;
use serde_json::Value;
use std::sync::{Mutex, MutexGuard};
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct ReviewedResourceAction {
    pub request: ResourceActionRequest,
    pub source_key: String,
    pub uid: String,
    pub resource_version: String,
    pub revision_identity: Option<(String, String)>,
    pub patch: Option<Value>,
    pub reviewed_after: Option<Value>,
}

pub(super) fn require_reviewed_after(reviewed_after: Option<&Value>) -> Result<&Value, AppError> {
    reviewed_after.ok_or_else(|| {
        AppError::new(
            "reviewed resource dry-run result is missing",
            AppErrorKind::Internal,
        )
    })
}

pub(super) fn ensure_dry_run_matches_review(
    reviewed_after: &Value,
    dry_run: &DynamicObject,
) -> Result<(), AppError> {
    let after = action_review_value(&action_value(dry_run)?);
    if reviewed_after != &after {
        return Err(AppError::new(
            "server dry-run result changed since the action was reviewed; review the action again",
            AppErrorKind::Validation,
        ));
    }
    Ok(())
}

pub(super) fn validate_review_guard(
    expected_uid: &str,
    expected_resource_version: &str,
    uid: &str,
    resource_version: &str,
) -> Result<(), AppError> {
    if expected_uid != uid || expected_resource_version != resource_version {
        return Err(AppError::new(
            "resource changed since the action was reviewed",
            AppErrorKind::Validation,
        ));
    }
    Ok(())
}

struct ReviewSlot {
    id: String,
    review: Option<ReviewedResourceAction>,
}

/// The one pending, single-use resource-action review owned by a backend session.
#[derive(Default)]
pub struct ResourceActionReviewStore {
    pending: Mutex<Option<ReviewSlot>>,
}

impl ResourceActionReviewStore {
    pub(super) fn begin(&self) -> Result<String, AppError> {
        let id = Uuid::new_v4().to_string();
        *self.lock()? = Some(ReviewSlot {
            id: id.clone(),
            review: None,
        });
        Ok(id)
    }

    pub(super) fn complete(
        &self,
        id: &str,
        review: ReviewedResourceAction,
    ) -> Result<(), AppError> {
        let mut pending = self.lock()?;
        let slot = pending
            .as_mut()
            .filter(|slot| slot.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "resource action review was superseded",
                    AppErrorKind::ConfirmationRequired,
                )
            })?;
        slot.review = Some(review);
        Ok(())
    }

    pub(super) fn discard(&self, id: &str) {
        if let Ok(mut pending) = self.pending.lock()
            && pending.as_ref().is_some_and(|slot| slot.id == id)
        {
            *pending = None;
        }
    }

    pub(super) fn consume(
        &self,
        confirmation: &ResourceActionConfirmation,
    ) -> Result<ReviewedResourceAction, AppError> {
        let mut pending = self.lock()?;
        let matches = pending
            .as_ref()
            .is_some_and(|slot| slot.id == confirmation.review_id && slot.review.is_some());
        if !matches {
            return Err(unavailable_review());
        }
        pending
            .take()
            .and_then(|slot| slot.review)
            .ok_or_else(unavailable_review)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Option<ReviewSlot>>, AppError> {
        self.pending.lock().map_err(|_| {
            AppError::new(
                "resource action review store is unavailable",
                AppErrorKind::Internal,
            )
        })
    }
}

fn unavailable_review() -> AppError {
    AppError::new(
        "resource action review is missing, superseded, or already used",
        AppErrorKind::ConfirmationRequired,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ClusterOperationTarget, ResourceActionKind};
    use std::sync::{Arc, Barrier};

    fn reviewed() -> ReviewedResourceAction {
        ReviewedResourceAction {
            request: ResourceActionRequest {
                target: ClusterOperationTarget {
                    cluster_context: "kind-dev".into(),
                    kind: "ConfigMap".into(),
                    name: "settings".into(),
                    api_version: Some("v1".into()),
                    namespace: Some("default".into()),
                },
                action: ResourceActionKind::Edit,
                edits: None,
                revision: None,
                kubeconfig_env_var: None,
            },
            source_key: "default".into(),
            uid: "uid-1".into(),
            resource_version: "7".into(),
            revision_identity: None,
            patch: Some(serde_json::json!({"metadata":{"labels":{"team":"platform"}}})),
            reviewed_after: Some(serde_json::json!({})),
        }
    }

    #[test]
    fn a_new_preview_supersedes_the_previous_review_even_if_it_finishes_first() {
        let store = ResourceActionReviewStore::default();
        let first = store.begin().unwrap();
        let second = store.begin().unwrap();
        assert!(store.complete(&first, reviewed()).is_err());
        store.complete(&second, reviewed()).unwrap();
        assert!(
            store
                .consume(&ResourceActionConfirmation { review_id: first })
                .is_err()
        );
        assert!(
            store
                .consume(&ResourceActionConfirmation { review_id: second })
                .is_ok()
        );
    }

    #[test]
    fn a_review_is_isolated_to_its_store() {
        let owner = ResourceActionReviewStore::default();
        let other = ResourceActionReviewStore::default();
        let id = owner.begin().unwrap();
        owner.complete(&id, reviewed()).unwrap();
        assert!(
            other
                .consume(&ResourceActionConfirmation {
                    review_id: id.clone(),
                })
                .is_err()
        );
        assert!(
            owner
                .consume(&ResourceActionConfirmation { review_id: id })
                .is_ok()
        );
    }

    #[test]
    fn concurrent_execution_consumes_a_review_exactly_once() {
        let store = Arc::new(ResourceActionReviewStore::default());
        let id = store.begin().unwrap();
        store.complete(&id, reviewed()).unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let handles = (0..2)
            .map(|_| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                let id = id.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .consume(&ResourceActionConfirmation { review_id: id })
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(|executed| *executed)
                .count(),
            1
        );
    }
}
