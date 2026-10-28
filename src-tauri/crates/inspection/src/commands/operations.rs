use kube::api::PatchParams;

/// Previews must never persist: every preview request goes to the API server
/// with `dryRun=All`. Centralized here so tests can pin the behavior.
pub(super) fn preview_patch_params() -> PatchParams {
    PatchParams::default().dry_run()
}
