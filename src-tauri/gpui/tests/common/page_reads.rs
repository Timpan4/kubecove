use super::*;
use kubecove_gpui::backend::{discovered_key, watch_signature};
use kubecove_lib::models::{StreamMessage, VersionedList, WatchResourceTarget};
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

    /// The GitOps inventory read, shaped by the scripted Argo inventory mode.
    pub(super) fn gitops_fixture(&self, context: String) -> Read<GitOpsInventory> {
        let mode = self.argo.inventory_mode.lock().unwrap().clone();
        let read = self
            .timing
            .wrap("detect_argocd", &context, self.page_gitops(&mode));
        Box::pin(async move {
            if mode == "unavailable" {
                return Err(AppError::new(
                    "Fixture provider detection unavailable",
                    AppErrorKind::Cluster,
                ));
            }
            let mut result = read.await?;
            match mode.as_str() {
                "absent" => {
                    result.argo = None;
                    result.flux = None;
                    result.app_sets.clear();
                    result.projects.clear();
                }
                "empty" => result.argo = Some(vec![]),
                "partial" => {
                    result.argo = Some(vec![]);
                    result.list_error = Some(AppError::new(
                        "Fixture Argo inventory unavailable",
                        AppErrorKind::Cluster,
                    ));
                }
                _ => {}
            }
            Ok(result)
        })
    }

    fn page_gitops(&self, mode: &str) -> Read<GitOpsInventory> {
        let phase = self.page_read_phase.lock().unwrap().clone();
        let appsets = self.page_reads.get("list_argocd_appsets").cloned();
        let flux = self.page_reads.get("list_flux_resources").cloned();
        let read = self.gitops.inventory(mode);
        // Every listed kind reports the mock list version, as tauri-dev-mock-argo.ts does.
        let version = self.list_version();
        let flux_keys: Vec<_> = self
            .kinds
            .iter()
            .filter(|kind| kind.group.ends_with("toolkit.fluxcd.io"))
            .map(|kind| discovered_key(kind, None))
            .collect();
        Box::pin(async move {
            if phase == "failed" {
                return Err(AppError::new(
                    "Scripted page refresh failure",
                    AppErrorKind::Network,
                ));
            }
            let mut data = read.await?;
            let argo_keys = kubecove_gpui::gitops::ARGO_KINDS
                .map(|(kind, plural)| kubecove_gpui::gitops::argo_watch_key(kind, plural));
            let mut listed = Vec::new();
            if data.argo.is_some() {
                listed.extend(&argo_keys);
            }
            if data.flux.is_some() {
                listed.extend(&flux_keys);
            }
            for key in listed {
                data.versions.insert(watch_signature(key), version.clone());
            }
            if phase == "updated"
                && let Some(value) = appsets
            {
                let list: VersionedList<_> =
                    serde_json::from_value(value).expect("exported ApplicationSets");
                data.app_sets = list.items;
            }
            if phase == "updated"
                && let Some(value) = flux
            {
                let list: VersionedList<_> =
                    serde_json::from_value(value).expect("exported Flux resources");
                data.flux = Some(list.items);
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
