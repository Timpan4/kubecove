use crate::commands::helpers::FluxOwnershipIndex;
use crate::models::ResourceSummary;
use serde::{Serialize, Serializer};
use std::{cmp::Ordering, sync::Arc};

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
            let shared = SharedRows::new(batches.clone(), &index, &[]);
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
                resource_versions: Vec::new(),
                failures: Vec::new(),
            };
            let expected_response = crate::models::ResourceScopeResult {
                resources: expected,
                completed_requests: Vec::new(),
                resource_versions: Vec::new(),
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

    #[test]
    fn shared_rows_keep_delimited_identity_order_and_last_duplicate() {
        let rows: Vec<ResourceSummary> = serde_json::from_value(json!([
            {"cluster": "a", "kind": "Pod", "name": "base", "age": "1m"},
            {"cluster": "a-b", "kind": "Pod", "name": "dash", "age": "1m"},
            {"cluster": "a0", "kind": "Pod", "name": "digit", "age": "1m"},
            {"cluster": "a:custom", "kind": "Pod", "name": "duplicate", "age": "1m"},
            {"cluster": "a", "kind": "custom:Pod", "name": "duplicate", "age": "2m"}
        ]))
        .unwrap();
        let shared = SharedRows::new(vec![Arc::new(rows)], &FluxOwnershipIndex::default(), &[]);
        assert_eq!(
            shared
                .iter()
                .map(|row| (row.name.as_str(), row.age.as_str()))
                .collect::<Vec<_>>(),
            [
                ("dash", "1m"),
                ("digit", "1m"),
                ("base", "1m"),
                ("duplicate", "2m")
            ],
        );
    }

    #[tokio::test]
    async fn shared_rows_match_owned_assembly_with_scoped_flux_inventory() {
        use crate::commands::helpers::fetch_flux_ownership_index;
        use http::{Request, Response};
        use kube::client::Body;

        let (service, mut handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
        let read = fetch_flux_ownership_index(kube::Client::new(service, "default"));
        let respond = async move {
            for _ in 0..2 {
                let (request, send) = handle.next_request().await.unwrap();
                let mut list = json!({"apiVersion": "v1", "kind": "List", "items": []});
                if request.uri().path().ends_with("/kustomizations") {
                    list["items"] = json!([{
                        "apiVersion": "kustomize.toolkit.fluxcd.io/v1",
                        "kind": "Kustomization",
                        "metadata": {"name": "apps", "namespace": "flux-system"},
                        "status": {"inventory": {"entries": [
                            {"id": "tenant_app_apps_Deployment", "v": "v1"},
                            {"id": "other_app_apps_Deployment", "v": "v1"},
                            {"id": "_shared_storage.k8s.io_StorageClass", "v": "v1"}
                        ]}}
                    }]);
                }
                send.send_response(Response::new(Body::from(
                    serde_json::to_vec(&list).unwrap(),
                )));
            }
        };
        let (index, ()) = tokio::join!(read, respond);
        let index = index.unwrap();
        assert_eq!(index.retained_item_count(), 3);

        let rows: Vec<ResourceSummary> = serde_json::from_value(json!([
            {"kind": "Deployment", "apiVersion": "apps/v1", "cluster": "test",
             "name": "app", "namespace": "tenant", "age": "1m"},
            {"kind": "Deployment", "apiVersion": "apps/v1", "cluster": "test",
             "name": "app", "namespace": "other", "age": "1m"},
            {"kind": "StorageClass", "apiVersion": "storage.k8s.io/v1", "cluster": "test",
             "name": "shared", "age": "1m"}
        ]))
        .unwrap();
        let namespaces = ["tenant".to_string()];
        let expected =
            super::super::assemble_post_fetch_scope(rows.clone(), index.clone(), &namespaces);
        let shared = SharedRows::new(vec![Arc::new(rows)], &index, &namespaces);
        assert_eq!(
            serde_json::to_vec(&shared).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
        assert_eq!(shared.changed.len(), 2);
        for row in shared.into_resources() {
            let owner_namespace = row
                .git_ops_owner
                .as_ref()
                .and_then(|owner| owner.namespace.as_deref());
            let expected_namespace =
                (row.namespace.as_deref() != Some("other")).then_some("flux-system");
            assert_eq!(owner_namespace, expected_namespace, "{}", row.name);
        }
    }
}

fn identity_parts(row: &ResourceSummary) -> [&str; 6] {
    [
        row.cluster.as_str(),
        row.kind.as_str(),
        row.api_version.as_deref().unwrap_or(""),
        row.plural.as_deref().unwrap_or(""),
        row.namespace.as_deref().unwrap_or(""),
        row.name.as_str(),
    ]
}

fn delimited_bytes<'a>(parts: &'a [&str]) -> impl Iterator<Item = u8> + 'a {
    parts
        .iter()
        .enumerate()
        .flat_map(|(index, part)| (index != 0).then_some(b':').into_iter().chain(part.bytes()))
}

fn compare_identity(left: &ResourceSummary, right: &ResourceSummary) -> Ordering {
    let left = identity_parts(left);
    let right = identity_parts(right);
    for (index, (&a, &b)) in left.iter().zip(&right).enumerate() {
        let ordering = a.cmp(b);
        if ordering.is_eq() {
            continue;
        }
        if index + 1 == left.len() || (!a.starts_with(b) && !b.starts_with(a)) {
            return ordering;
        }
        // A separator can sort before or after the next byte of the longer field.
        return delimited_bytes(&left[index..]).cmp(delimited_bytes(&right[index..]));
    }
    Ordering::Equal
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
    pub(super) fn new(
        batches: Vec<Arc<Vec<ResourceSummary>>>,
        index: &FluxOwnershipIndex,
        namespaces: &[String],
    ) -> Self {
        let mut unique = Vec::with_capacity(batches.iter().map(|batch| batch.len()).sum());
        for (batch_index, batch) in batches.iter().enumerate() {
            for (row_index, row) in batch.iter().enumerate() {
                unique.push((row, (batch_index, row_index)));
            }
        }
        // For equal keys, put the last row first so deduplication retains it.
        unique.sort_unstable_by(|a, b| compare_identity(a.0, b.0).then_with(|| b.1.cmp(&a.1)));
        unique.dedup_by(|a, b| compare_identity(a.0, b.0).is_eq());
        let mut rows = Vec::with_capacity(unique.len());
        let mut changed = Vec::new();
        for (summary, (batch, row)) in unique {
            if let Some(summary) = index.enriched_summary(summary, namespaces) {
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

    pub(super) fn iter(&self) -> impl ExactSizeIterator<Item = &ResourceSummary> {
        self.rows.iter().map(|row| self.get(row))
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
        serializer.collect_seq(self.iter())
    }
}
