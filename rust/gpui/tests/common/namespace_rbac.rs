use super::Fixture;
use kubecove_gpui::backend::Read;
use kubecove_lib::models::{
    AppError, AppErrorKind, NamespaceSummary, RbacAccessReviewRequest, RbacAccessReviewResult,
    RbacAccessReviewTarget, RbacInspectionSummary,
};

impl Fixture {
    pub(super) fn namespaces_read(&self, context: String) -> Read<Vec<NamespaceSummary>> {
        let phase = self.shell_read_phase.lock().unwrap().clone();
        if phase == "namespaces-failed" {
            return Box::pin(async {
                Err(AppError::new(
                    "Scripted namespaces read failure",
                    AppErrorKind::Cluster,
                ))
            });
        }
        // Typed like the browser mock's `namespaces-credential-failed` phase.
        if phase == "namespaces-credential-failed" {
            return Box::pin(async {
                Err(AppError::new(
                    "Scripted read: credentialUnavailable",
                    AppErrorKind::CredentialUnavailable,
                ))
            });
        }
        let namespaces = if phase == "namespaces-empty" {
            Vec::new()
        } else {
            self.namespaces.clone()
        };
        self.timing.wrap(
            "list_namespaces",
            &context,
            Box::pin(async move { Ok(namespaces) }),
        )
    }

    pub(super) fn rbac_read(&self, context: String) -> Read<RbacInspectionSummary> {
        let found = self.rbac.get(&context).cloned();
        self.coverage_read(
            "list_rbac_inspection",
            self.page_read(
                "list_rbac_inspection",
                self.timing.wrap(
                    "list_rbac_inspection",
                    &context,
                    Box::pin(async move {
                        found.ok_or_else(|| {
                            AppError::new("No fixture RBAC inspection", AppErrorKind::NotFound)
                        })
                    }),
                ),
            ),
        )
    }

    pub(super) fn review_rbac_read(
        &self,
        request: RbacAccessReviewRequest,
    ) -> Read<RbacAccessReviewResult> {
        let verb = match &request.target {
            RbacAccessReviewTarget::Resource { verb, .. }
            | RbacAccessReviewTarget::NonResource { verb, .. } => verb.trim().to_lowercase(),
        };
        let found = self
            .rbac_reviews
            .iter()
            .find(|review| review.verb == verb)
            .map(|review| (review.result.clone(), review.error.clone()));
        Box::pin(async move {
            match found {
                Some((Some(result), _)) => Ok(result),
                Some((None, Some(error))) => Err(AppError::new(error, AppErrorKind::Cluster)),
                _ => Err(AppError::new(
                    format!("No fixture review for {verb}"),
                    AppErrorKind::NotFound,
                )),
            }
        })
    }
}
