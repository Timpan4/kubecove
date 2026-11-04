//! Native reads for the RBAC Cockpit and the Kubeconfig settings section,
//! through the shared typed commands.

use kubecove_lib::{
    commands::{self, BackendCancellationRegistry, kubeconfig::KubeconfigSourcesSummary},
    models::{
        AppError, AppErrorKind, RbacAccessReviewRequest, RbacAccessReviewResult,
        RbacInspectionSummary,
    },
    runtime::State,
};

/// The kubeconfig source commands persist only after the desktop host calls
/// `init_kubeconfig_settings_path`. The native client does not, so a write
/// would return success without saving; the settings page disables them.
pub(crate) const KUBECONFIG_WRITES: &str = "Saved in the desktop app's kubeconfig settings file, which the native client does not load yet.";

/// `RbacSurface.svelte`'s `listRbacInspection` query.
pub(super) async fn rbac(
    context: String,
    cancellations: BackendCancellationRegistry,
) -> Result<RbacInspectionSummary, AppError> {
    commands::list_rbac_inspection(context, None, None, None, State::new(&cancellations)).await
}

/// `RbacView.svelte`'s `verify()`.
pub(super) async fn review(
    request: RbacAccessReviewRequest,
    cancellations: BackendCancellationRegistry,
) -> Result<RbacAccessReviewResult, AppError> {
    commands::review_rbac_access(request, None, None, State::new(&cancellations)).await
}

pub(super) async fn kubeconfig_sources() -> Result<KubeconfigSourcesSummary, AppError> {
    commands::get_kubeconfig_sources()
}

pub(super) async fn unsaved() -> Result<KubeconfigSourcesSummary, AppError> {
    Err(AppError::new(
        KUBECONFIG_WRITES,
        AppErrorKind::UnsupportedOperation,
    ))
}
