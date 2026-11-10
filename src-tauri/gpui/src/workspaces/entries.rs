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

    /// `entryPointKey` equality with a live row, API version included.
    pub fn identifies(&self, row: &ResourceSummary) -> bool {
        self.kind == EntryKind::Resource
            && self.cluster_context == row.cluster
            && self.namespace == row.namespace
            && self.name == row.name
            && self.resource_kind.as_ref() == Some(&row.kind)
            && self.api_version == row.api_version
    }

    /// `resourceRequestCoversEntry`: a completed list in this entry's context
    /// and namespace would have returned it. A built-in list covers every API
    /// version of its kind; a discovered list covers its own version.
    pub fn listed_by(&self, listed: &crate::backend::Listed) -> bool {
        let kind = &listed.key.resource_kind;
        self.kind == EntryKind::Resource
            && self.cluster_context == listed.context
            && self.resource_kind.as_ref() == Some(&kind.kind)
            && listed
                .key
                .namespace
                .as_ref()
                .is_none_or(|namespace| self.namespace.as_ref() == Some(namespace))
            && (kind.api_version.is_none()
                || self.api_version.is_none()
                || self.api_version == kind.api_version)
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
