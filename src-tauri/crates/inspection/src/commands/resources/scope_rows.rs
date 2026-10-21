use crate::commands::helpers::{enrich_resource_summaries_with_flux_inventory, FluxOwnershipIndex};
use crate::models::ResourceSummary;
use serde::{Serialize, Serializer};
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shared_rows_preserve_owned_assembly_bytes_and_only_copy_changed_rows() {
        // Failure modes: reordered rows, first/last duplicate precedence, lost
        // partial ownership flags, or copying unchanged cached rows.
        let first:ResourceSummary = serde_json::from_value(json!({"kind":"Pod", "cluster":"context", "name":"app", "namespace":"tenant", "age":"1m", "status":"Running", "restarts":2})).unwrap();
        let mut replacement = first.clone();
        replacement.status = Some("Pending".into());
        let mut other = first.clone();
        other.name = "another".into();
        let batches = vec![Arc::new(vec![first, other]), Arc::new(vec![replacement])];
        for partial in [false, true] {
            let mut index = FluxOwnershipIndex::default();
            index.partial = partial;
            let expected = super::super::assemble_post_fetch_scope(
                batches
                    .iter()
                    .flat_map(|batch| batch.iter().cloned())
                    .collect(),
                index.clone(),
                &[],
            );
            let shared = SharedRows::new(batches.clone(), &index);
            assert_eq!(
                serde_json::to_vec(&shared).unwrap(),
                serde_json::to_vec(&expected).unwrap()
            );
            assert_eq!(
                shared.changed.len(),
                if partial { expected.len() } else { 0 }
            );
            let response = super::super::ResourceScopeResponse {
                resources: shared,
                completed_requests: Vec::new(),
                failures: Vec::new(),
            };
            let expected_response = crate::models::ResourceScopeResult {
                resources: expected,
                completed_requests: Vec::new(),
                failures: Vec::new(),
            };
            assert_eq!(
                serde_json::to_vec(&response).unwrap(),
                serde_json::to_vec(&expected_response).unwrap()
            );
            assert_eq!(
                serde_json::to_vec(&response.resources.into_resources()).unwrap(),
                serde_json::to_vec(&expected_response.resources).unwrap()
            );
        }
        assert!(!batches[0][0].git_ops_ownership_partial);
    }
}

enum RowRef {
    Cached(usize, usize),
    Changed(usize),
}

#[derive(Default)]
pub struct SharedRows {
    batches: Vec<Arc<Vec<ResourceSummary>>>,
    rows: Vec<RowRef>,
    changed: Vec<ResourceSummary>,
}

impl SharedRows {
    pub(super) fn new(batches: Vec<Arc<Vec<ResourceSummary>>>, index: &FluxOwnershipIndex) -> Self {
        let mut unique = BTreeMap::new();
        for (batch_index, batch) in batches.iter().enumerate() {
            for (row_index, row) in batch.iter().enumerate() {
                unique.insert(super::row_key(row), (batch_index, row_index));
            }
        }
        let mut rows = Vec::with_capacity(unique.len());
        let mut changed = Vec::new();
        for (batch, row) in unique.into_values() {
            let summary = &batches[batch][row];
            if index.would_enrich(summary) {
                let mut summary = summary.clone();
                enrich_resource_summaries_with_flux_inventory(
                    std::slice::from_mut(&mut summary),
                    index,
                );
                rows.push(RowRef::Changed(changed.len()));
                changed.push(summary);
            } else {
                rows.push(RowRef::Cached(batch, row));
            }
        }
        Self {
            batches,
            rows,
            changed,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, row: &RowRef) -> &ResourceSummary {
        match *row {
            RowRef::Cached(batch, row) => &self.batches[batch][row],
            RowRef::Changed(row) => &self.changed[row],
        }
    }

    pub(super) fn into_resources(self) -> Vec<ResourceSummary> {
        let Self {
            batches,
            rows,
            changed,
        } = self;
        let mut changed = changed.into_iter();
        rows.into_iter()
            .map(|row| match row {
                RowRef::Cached(batch, row) => batches[batch][row].clone(),
                // Changed rows are appended in the same order as their references.
                RowRef::Changed(_) => changed.next().expect("changed scope row"),
            })
            .collect()
    }
}

impl Serialize for SharedRows {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.rows.iter().map(|row| self.get(row)))
    }
}
