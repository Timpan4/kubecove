use super::*;

impl Fixture {
    /// The kubeconfig source read that the launcher refetches on open and Retry. The
    /// `sources-failed` phase fails it, as the web's mock does for `get_kubeconfig_sources`.
    pub(super) fn launcher_kubeconfig_sources(&self) -> Read<KubeconfigSourcesSummary> {
        if *self.shell_read_phase.lock().unwrap() == "sources-failed" {
            return Box::pin(async {
                Err(AppError::new(
                    "Scripted kubeconfig sources read failure",
                    AppErrorKind::Cluster,
                ))
            });
        }
        self.kubeconfig_sources_read()
    }
}
