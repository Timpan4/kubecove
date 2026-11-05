//! Hydration and save ownership stay at the window's Backend seam.
use super::{Inspector, delivery_ui::ViewMode, detail::Detail};
use crate::preferences::{DeliveryMode, YamlFormat, YamlShape};
use gpui_kit::Context;

impl Inspector {
    pub(super) fn preset_key(&self) -> String {
        let source = self
            .sessions
            .load
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .and_then(|s| s.source_key.as_deref());
        serde_json::to_string(&(source, &self.scope.context, self.workspace_name.as_ref()))
            .expect("string tuple")
    }

    pub(super) fn ensure_active_workspace(&mut self) {
        if self.launcher.active.is_none() && !self.scope.context.is_empty() {
            if let Some(saved) = self.launcher.saved.iter().find(|s| {
                s.name.as_str() == self.workspace_name.as_ref() && s.context == self.scope.context
            }) {
                self.launcher.active = Some(saved.id);
                self.rbac.reviews = saved.reviews.clone();
                return;
            }
            // Saved records may have been renamed since the startup view was named.
            // Opening the launcher must not recreate that old name.
            if !self.launcher.saved.is_empty() {
                return;
            }
            let id = self.launcher.next;
            self.launcher.next = self.launcher.next.saturating_add(1);
            self.launcher.saved.push(
                crate::workspaces::Draft {
                    name: &self.workspace_name,
                    effective: &self.scope.context,
                    group: &[],
                    namespaces: &[],
                    kinds: None,
                }
                .build(id, id),
            );
            self.launcher.active = Some(id);
        }
    }

    pub(super) fn timestamp_label(&self, relative: &str, value: Option<&str>) -> String {
        if !self.persisted.preferences.show_exact_timestamps {
            return relative.to_owned();
        }
        let Some(time) = value.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()) else {
            return relative.to_owned();
        };
        let exact = match self.persisted.preferences.timestamp_timezone {
            crate::preferences::TimestampTimezone::Utc => time
                .with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
            crate::preferences::TimestampTimezone::Local => time
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S %Z")
                .to_string(),
        };
        format!("{relative} ({exact})")
    }

    pub(super) fn load_preferences(&mut self, cx: &mut Context<Self>) {
        let document = match self.backend.load_preferences() {
            Ok(document) => {
                self.persistence_error = None;
                document
            }
            Err(error) => {
                self.persistence_error = Some(error.message);
                Default::default()
            }
        };
        self.launcher.saved = document.workspaces.clone();
        self.launcher.next = self
            .launcher
            .saved
            .iter()
            .map(|s| s.id.max(s.updated))
            .max()
            .unwrap_or(0)
            + 1;
        self.launcher.transfer.documents =
            document.workspace_documents.clone().into_iter().collect();
        self.persisted = document;
        let lanes = &self.persisted.preferences.resource_kind_lanes;
        self.settings.kind_focus = lanes
            .first
            .iter()
            .chain(&lanes.normal)
            .chain(&lanes.last)
            .map(|kind| (kind.clone(), cx.focus_handle()))
            .collect();
        self.refresh_diagnostics(false, cx);
        self.timings
            .set_enabled(self.persisted.preferences.debug_mode_enabled);
        self.argo_connections.profiles.clear();
        self.argo_connections.scope.clear();
        self.argo_outcomes.clear();
        self.restore_argo_preferences();
        if self.usage.visible != self.persisted.preferences.show_usage_footer {
            self.toggle_usage(cx);
        }
        self.gitops_page.mode = view(self.persisted.preferences.git_ops_view_mode);
        self.helm_page.mode = view(self.persisted.preferences.helm_view_mode);
    }

    pub(super) fn save_preferences(&mut self, cx: &mut Context<Self>) {
        self.sync_workspace_reviews();
        self.persisted.workspaces = self.launcher.saved.clone();
        self.persisted
            .workspace_entry_points
            .retain(|id, _| self.launcher.saved.iter().any(|saved| saved.id == *id));
        self.persisted.workspace_documents = self
            .launcher
            .transfer
            .documents
            .iter()
            .filter(|(id, _)| self.launcher.saved.iter().any(|s| s.id == **id))
            .map(|(id, doc)| (*id, doc.clone()))
            .collect();
        match self.backend.save_preferences(self.persisted.clone()) {
            Ok(document) => {
                self.persisted = document;
                self.persistence_error = None;
            }
            Err(error) => self.persistence_error = Some(error.message),
        }
        cx.notify();
    }
    pub(super) fn apply_yaml_defaults(
        &mut self,
        cx: &mut Context<Self>,
        window: &mut gpui_kit::Window,
    ) {
        if let Detail::Ready(detail) = &mut self.detail {
            let prefs = &self.persisted.preferences;
            detail.yaml_view.mode = match prefs.yaml_view_mode_default {
                YamlShape::Kubectl => kubecove_lib::models::YamlViewMode::Kubectl,
                YamlShape::ApplyClean => kubecove_lib::models::YamlViewMode::ApplyClean,
            };
            detail.yaml_view.format = match prefs.yaml_encoding_default {
                YamlFormat::Yaml => kubecove_lib::models::YamlEncoding::Yaml,
                YamlFormat::Kyaml => kubecove_lib::models::YamlEncoding::Kyaml,
            };
            let shape = if prefs.yaml_view_mode_default == YamlShape::ApplyClean {
                "Apply clean"
            } else {
                "Kubectl view"
            };
            let encoding = if prefs.yaml_encoding_default == YamlFormat::Kyaml {
                "KYAML"
            } else {
                "YAML"
            };
            detail.yaml_view.shape.update(cx, |picker, cx| {
                picker.set_selected_value(&shape.into(), window, cx)
            });
            detail.yaml_view.encoding.update(cx, |picker, cx| {
                picker.set_selected_value(&encoding.into(), window, cx)
            });
        }
    }
}
fn view(mode: DeliveryMode) -> ViewMode {
    match mode {
        DeliveryMode::Cards => ViewMode::Cards,
        DeliveryMode::List => ViewMode::List,
    }
}
