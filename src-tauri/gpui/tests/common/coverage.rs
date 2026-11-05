use super::*;
use serde::de::DeserializeOwned;

#[derive(Default)]
pub(super) struct CoverageState {
    script: Mutex<String>,
    pending: Mutex<Vec<tokio::sync::oneshot::Sender<String>>>,
}

impl Fixture {
    // Other fixture harnesses do not inject a subsequent log message.
    #[allow(dead_code)]
    pub fn emit_coverage_log(&self) {
        self.detail_tabs.emit_log();
    }
    pub(super) fn coverage_script(&self) -> String {
        self.coverage_state.script.lock().unwrap().clone()
    }
    // The native and visual harnesses share the fixture but do not drive coverage scripts.
    #[allow(dead_code)]
    pub fn configure_coverage(&self, script: &str) {
        *self.coverage_state.script.lock().unwrap() = script.into();
        for sender in self.coverage_state.pending.lock().unwrap().drain(..) {
            let _ = sender.send(script.into());
        }
    }

    pub(super) fn coverage_read<T: DeserializeOwned + Send + 'static>(
        &self,
        command: &str,
        base: Read<T>,
    ) -> Read<T> {
        let script = self.coverage_state.script.lock().unwrap().clone();
        if script == "startup-failed" && command == "get_kubeconfig_sources" {
            return Box::pin(async {
                Err(AppError::new(
                    "Controlled source read failure",
                    AppErrorKind::Cluster,
                ))
            });
        }
        if (script == "loading" && command != "get_kubeconfig_sources")
            || (script == "detail-loading" && command == "get_helm_release_details")
            || (script == "enrichment-loading"
                && matches!(command, "get_resource_details" | "list_resource_topology"))
        {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            self.coverage_state.pending.lock().unwrap().push(sender);
            return Box::pin(async move {
                if receiver.await.ok().as_deref() == Some("failed") {
                    return Err(AppError::new(
                        "Controlled inspection failure",
                        AppErrorKind::Cluster,
                    ));
                }
                base.await
            });
        }
        let entry = self
            .coverage
            .get(&script)
            .and_then(|entries| entries.get(command));
        let Some(entry) = entry else {
            return base;
        };
        let error = entry
            .get("error")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let result = entry.get("result").cloned();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(AppError::new(error, AppErrorKind::Cluster));
            }
            if let Some(result) = result {
                return Ok(serde_json::from_value(result).expect("exported coverage response"));
            }
            base.await
        })
    }
}
