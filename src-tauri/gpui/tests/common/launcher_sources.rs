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

    /// The context list the shell reads at startup, as the shell read phase scripts it.
    pub(super) fn launcher_contexts(&self) -> Read<Vec<ClusterContext>> {
        if *self.shell_read_phase.lock().unwrap() == "contexts-failed" {
            return Box::pin(async {
                Err(AppError::new(
                    "Scripted contexts read failure",
                    AppErrorKind::Cluster,
                ))
            });
        }
        if *self.shell_read_phase.lock().unwrap() == "contexts-empty" {
            return Box::pin(async { Ok(vec![]) });
        }
        // The saved primary context of the parity workspaces is mock-dev.
        let missing = *self.shell_read_phase.lock().unwrap() == "primary-context-missing";
        let available: Vec<_> = self
            .cluster_contexts
            .iter()
            .filter(|context| !(missing && context.name == "mock-dev"))
            .cloned()
            .collect();
        // The web mock keys `list_kube_contexts` without a cluster context.
        self.timing.wrap(
            "list_kube_contexts",
            "",
            Box::pin(async move { Ok(available) }),
        )
    }
}
