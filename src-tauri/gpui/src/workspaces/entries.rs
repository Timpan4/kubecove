//! Only entry identities persist. Live resource contents never enter this model.
use kubecove_lib::models::ResourceSummary;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    Namespace,
    App,
    Resource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub kind: EntryKind,
    pub cluster_context: String,
    pub namespace: Option<String>,
    pub name: String,
    pub resource_kind: Option<String>,
    pub api_version: Option<String>,
    pub last_visited_at: String,
}

impl Entry {
    pub fn resource(resource: &ResourceSummary) -> Self {
        Self {
            kind: EntryKind::Resource,
            cluster_context: resource.cluster.clone(),
            namespace: resource.namespace.clone(),
            name: resource.name.clone(),
            resource_kind: Some(resource.kind.clone()),
            api_version: resource.api_version.clone(),
            last_visited_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn same_target(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.cluster_context == other.cluster_context
            && self.namespace == other.namespace
            && self.name == other.name
            && self.resource_kind == other.resource_kind
            && self.api_version == other.api_version
    }

    pub fn valid(&self) -> bool {
        !self.cluster_context.trim().is_empty()
            && !self.name.trim().is_empty()
            && self
                .namespace
                .as_ref()
                .is_none_or(|value| !value.trim().is_empty())
            && (self.kind != EntryKind::Resource
                || self
                    .resource_kind
                    .as_ref()
                    .is_some_and(|value| !value.trim().is_empty()))
            && chrono::DateTime::parse_from_rfc3339(&self.last_visited_at).is_ok()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Entries {
    pub pinned: Vec<Entry>,
    pub recent: Vec<Entry>,
}

impl Entries {
    pub fn pinned(&self, entry: &Entry) -> bool {
        self.pinned.iter().any(|item| item.same_target(entry))
    }
    pub fn toggle_pin(&mut self, entry: Entry) {
        if self.pinned(&entry) {
            self.pinned.retain(|item| !item.same_target(&entry));
        } else {
            self.pinned.insert(0, entry);
        }
    }
    /// Drops resource entries that `missing` reports as absent from a completed
    /// read. Returns whether any entry was dropped.
    pub fn drop_missing(&mut self, mut missing: impl FnMut(&Entry) -> bool) -> bool {
        let before = self.pinned.len() + self.recent.len();
        let mut keep = |entry: &Entry| entry.kind != EntryKind::Resource || !missing(entry);
        self.pinned.retain(|entry| keep(entry));
        self.recent.retain(|entry| keep(entry));
        before != self.pinned.len() + self.recent.len()
    }
    pub fn visit(&mut self, entry: Entry) {
        self.recent.retain(|item| !item.same_target(&entry));
        self.recent.insert(0, entry);
        // RECENT_ENTRY_LIMIT in src/lib/workspace-entry-points.ts.
        self.recent.truncate(8);
    }
}
