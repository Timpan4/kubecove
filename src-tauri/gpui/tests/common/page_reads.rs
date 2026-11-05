use super::*;
use kubecove_lib::models::{StreamMessage, WatchResourceTarget};
use serde::de::DeserializeOwned;

impl Fixture {
    // Shared fixture module; only the parity test crate drives replay.
    #[allow(dead_code)]
    pub fn replay_page_reads(&self, phase: &str) {
        *self.page_read_phase.lock().unwrap() = phase.into();
        self.live.page_change();
    }

    pub(super) fn page_read<T: DeserializeOwned + Send + 'static>(
        &self,
        command: &str,
        base: Read<T>,
    ) -> Read<T> {
        let phase = self.page_read_phase.lock().unwrap().clone();
        let updated = self.page_reads.get(command).cloned();
        Box::pin(async move {
            if phase == "failed" {
                return Err(AppError::new(
                    "Scripted page refresh failure",
                    AppErrorKind::Network,
                ));
            }
            if phase == "updated"
                && let Some(value) = updated
            {
                return Ok(serde_json::from_value(value).expect("exported page response"));
            }
            base.await
        })
    }

    pub(super) fn page_gitops(&self) -> Read<GitOpsInventory> {
        let phase = self.page_read_phase.lock().unwrap().clone();
        let appsets = self.page_reads.get("list_argocd_appsets").cloned();
        let flux = self.page_reads.get("list_flux_resources").cloned();
        let read = self.gitops.inventory();
        Box::pin(async move {
            if phase == "failed" {
                return Err(AppError::new(
                    "Scripted page refresh failure",
                    AppErrorKind::Network,
                ));
            }
            let mut data = read.await?;
            if phase == "updated"
                && let Some(value) = appsets
            {
                data.app_sets = serde_json::from_value(value).expect("exported ApplicationSets");
            }
            if phase == "updated"
                && let Some(value) = flux
            {
                data.flux = Some(serde_json::from_value(value).expect("exported Flux resources"));
            }
            Ok(data)
        })
    }
}

impl super::live_data::LiveData {
    // Shared fixture module; only the parity test crate drives replay.
    #[allow(dead_code)]
    pub(super) fn page_change(&self) {
        let mut state = self.state.lock().unwrap();
        state.watches.retain(|(_, sender)| !sender.is_closed());
        for (target, sender) in &state.watches {
            if let kubecove_gpui::backend::WatchTarget::Resource { key, .. } = target
                && matches!(
                    key.resource_kind.kind.as_str(),
                    "ApplicationSet" | "Kustomization"
                )
            {
                let _ = sender.send(StreamMessage::ResourceChanged {
                    stream_id: "fixture".into(),
                    target: WatchResourceTarget {
                        cluster: "mock-dev".into(),
                        kind: key.resource_kind.kind.clone(),
                        namespace: None,
                        name: None,
                    },
                    action: "applied".into(),
                });
            }
        }
    }
}
