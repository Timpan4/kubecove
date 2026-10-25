use super::{fixed_now, planning, summaries, topology};
use crate::{
    commands::{live_store::ClusterLiveStore, resources},
    models::{AppError, ResourceSummary, WatchResourceKind},
};
use std::sync::Arc;

const SOURCE: &str = "cache-catalog-source";
const CONTEXT: &str = "cache-catalog-context";

pub struct ResourceCacheFixture {
    store: ClusterLiveStore,
    rows: Vec<ResourceSummary>,
    replacement: Vec<ResourceSummary>,
    topology: crate::models::ResourceTopology,
}

pub fn resource_cache_fixture() -> ResourceCacheFixture {
    let summary_fixture = summaries::resource_summary_fixture(10_000);
    let rows = summary_fixture
        .pods
        .iter()
        .map(|pod| resources::builtin_summary(CONTEXT, pod, fixed_now()))
        .collect::<Vec<_>>();
    let mut replacement = rows.clone();
    if let Some(first) = replacement.first_mut() {
        first.status = Some("Running after watch replacement".to_string());
    }
    let topology_fixture = topology::ownership_topology_fixture();
    let topology = topology::build_ownership_topology(topology_fixture);
    let index_fixture = planning::flux_index_fixture();
    let index = planning::build_flux_index_inner(index_fixture);
    let store = ClusterLiveStore::default();
    let resources_seed = rows.clone();
    let topology_seed = topology.clone();
    let index_seed = index;
    tauri::async_runtime::block_on(async {
        store
            .typed_resources(
                SOURCE.to_string(),
                CONTEXT.to_string(),
                "Pod".to_string(),
                None,
                move || async move { Ok::<_, AppError>(resources_seed) },
            )
            .await
            .expect("seed ready resources");
        store
            .topology(
                SOURCE.to_string(),
                CONTEXT.to_string(),
                Vec::new(),
                "ownership".to_string(),
                move || async move { Ok::<_, AppError>(topology_seed) },
            )
            .await
            .expect("seed ready topology");
        store
            .flux_ownership_index(
                SOURCE.to_string(),
                CONTEXT.to_string(),
                move || async move { Ok::<_, AppError>(index_seed) },
            )
            .await
            .expect("seed ready Flux index");
    });
    ResourceCacheFixture {
        store,
        rows,
        replacement,
        topology,
    }
}

impl ResourceCacheFixture {
    pub fn assert_cache_partitioning_and_namespace_normalization(&self) {
        let other_source = "cache-catalog-other-source";
        let other_context = "cache-catalog-other-context";

        assert!(tauri::async_runtime::block_on(self.store.typed_resources(
            other_source.to_string(),
            CONTEXT.to_string(),
            "Pod".to_string(),
            None,
            || async { Err(cache_isolation_error()) },
        ))
        .is_err());
        assert!(tauri::async_runtime::block_on(self.store.typed_resources(
            SOURCE.to_string(),
            other_context.to_string(),
            "Pod".to_string(),
            None,
            || async { Err(cache_isolation_error()) },
        ))
        .is_err());
        assert!(tauri::async_runtime::block_on(self.store.topology(
            other_source.to_string(),
            CONTEXT.to_string(),
            Vec::new(),
            "ownership".to_string(),
            || async { Err(cache_isolation_error()) },
        ))
        .is_err());
        assert!(tauri::async_runtime::block_on(self.store.topology(
            SOURCE.to_string(),
            other_context.to_string(),
            Vec::new(),
            "ownership".to_string(),
            || async { Err(cache_isolation_error()) },
        ))
        .is_err());
        assert!(tauri::async_runtime::block_on(self.store.topology(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            Vec::new(),
            "different-mode".to_string(),
            || async { Err(cache_isolation_error()) },
        ))
        .is_err());
        assert!(
            tauri::async_runtime::block_on(self.store.flux_ownership_index(
                other_source.to_string(),
                CONTEXT.to_string(),
                || async { Err(cache_isolation_error()) },
            ))
            .is_err()
        );
        assert!(
            tauri::async_runtime::block_on(self.store.flux_ownership_index(
                SOURCE.to_string(),
                other_context.to_string(),
                || async { Err(cache_isolation_error()) },
            ))
            .is_err()
        );

        self.assert_topology_namespace_normalization();
        self.assert_present_kind_namespace_normalization();
        assert!(self
            .store
            .cached_present_custom_resource_kinds(other_source, CONTEXT, &[])
            .is_none());
        assert!(self
            .store
            .cached_present_custom_resource_kinds(SOURCE, other_context, &[])
            .is_none());
    }

    fn assert_topology_namespace_normalization(&self) {
        let seeded_namespaces = vec![
            "tenant-42".to_string(),
            "tenant-7".to_string(),
            "tenant-42".to_string(),
        ];
        let lookup_namespaces = vec!["tenant-7".to_string(), "tenant-42".to_string()];
        let topology = self.topology.clone();
        let expected_nodes = topology.nodes.len();
        tauri::async_runtime::block_on(self.store.topology(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            seeded_namespaces,
            "namespace-normalization-check".to_string(),
            move || async move { Ok::<_, AppError>(topology) },
        ))
        .expect("seed normalized topology cache key");
        let hit = tauri::async_runtime::block_on(self.store.topology(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            lookup_namespaces,
            "namespace-normalization-check".to_string(),
            || async { Err(cache_isolation_error()) },
        ))
        .expect("reordered and deduplicated topology namespace key hits");
        assert_eq!(hit.nodes.len(), expected_nodes);

        let topology = self.topology.clone();
        tauri::async_runtime::block_on(self.store.topology(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            Vec::new(),
            "empty-namespace-normalization-check".to_string(),
            move || async move { Ok::<_, AppError>(topology) },
        ))
        .expect("seed empty topology namespace key");
        let hit = tauri::async_runtime::block_on(self.store.topology(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            Vec::new(),
            "empty-namespace-normalization-check".to_string(),
            || async { Err(cache_isolation_error()) },
        ))
        .expect("empty topology namespace key hits");
        assert_eq!(hit.nodes.len(), expected_nodes);
    }

    fn assert_present_kind_namespace_normalization(&self) {
        let seeded_namespaces = vec![
            "tenant-42".to_string(),
            "tenant-7".to_string(),
            "tenant-42".to_string(),
        ];
        tauri::async_runtime::block_on(self.store.present_custom_resource_kinds(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            seeded_namespaces,
            || async { Ok(Vec::new()) },
        ))
        .expect("seed normalized present-kind cache key");
        let reordered = ["tenant-7".to_string(), "tenant-42".to_string()];
        assert!(self
            .store
            .cached_present_custom_resource_kinds(SOURCE, CONTEXT, &reordered)
            .is_some_and(|kinds| kinds.is_empty()));

        tauri::async_runtime::block_on(self.store.present_custom_resource_kinds(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            Vec::new(),
            || async { Ok(Vec::new()) },
        ))
        .expect("seed empty present-kind cache key");
        assert!(self
            .store
            .cached_present_custom_resource_kinds(SOURCE, CONTEXT, &[])
            .is_some_and(|kinds| kinds.is_empty()));
    }

    pub fn ready_resource_hit(&self) -> Result<Arc<Vec<ResourceSummary>>, AppError> {
        tauri::async_runtime::block_on(self.store.typed_resources(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            "Pod".to_string(),
            None,
            || async {
                Err(AppError::new(
                    "ready cache hit unexpectedly loaded",
                    crate::models::AppErrorKind::Cluster,
                ))
            },
        ))
    }

    pub fn named_scope_from_all_hit(&self) -> Result<Arc<Vec<ResourceSummary>>, AppError> {
        tauri::async_runtime::block_on(self.store.typed_resources(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            "Pod".to_string(),
            Some("tenant-42".to_string()),
            || async {
                Err(AppError::new(
                    "all-scope hit unexpectedly loaded",
                    crate::models::AppErrorKind::Cluster,
                ))
            },
        ))
    }

    pub fn topology_hit(&self) -> Result<Arc<crate::models::ResourceTopology>, AppError> {
        tauri::async_runtime::block_on(self.store.topology(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            Vec::new(),
            "ownership".to_string(),
            || async {
                Err(AppError::new(
                    "topology hit unexpectedly loaded",
                    crate::models::AppErrorKind::Cluster,
                ))
            },
        ))
    }

    pub fn flux_index_hit(&self) -> Result<usize, AppError> {
        tauri::async_runtime::block_on(self.store.flux_ownership_index(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            || async {
                Err(AppError::new(
                    "Flux index hit unexpectedly loaded",
                    crate::models::AppErrorKind::Cluster,
                ))
            },
        ))
        .map(|index| index.retained_item_count())
    }

    pub fn dirty_replacement(&mut self) -> Result<Arc<Vec<ResourceSummary>>, AppError> {
        let kind = WatchResourceKind {
            kind: "Pod".to_string(),
            group: None,
            version: Some("v1".to_string()),
            api_version: Some("v1".to_string()),
            plural: Some("pods".to_string()),
            namespaced: Some(true),
        };
        self.store
            .mark_watch_resource_dirty(SOURCE, CONTEXT, &kind, None);
        let replacement = std::mem::take(&mut self.replacement);
        tauri::async_runtime::block_on(self.store.typed_resources(
            SOURCE.to_string(),
            CONTEXT.to_string(),
            "Pod".to_string(),
            None,
            move || async move { Ok::<_, AppError>(replacement) },
        ))
    }

    pub fn expected_rows(&self) -> &[ResourceSummary] {
        &self.rows
    }

    pub fn expected_topology(&self) -> &crate::models::ResourceTopology {
        &self.topology
    }
}

fn cache_isolation_error() -> AppError {
    AppError::new(
        "isolated cache-key probe reached its loader",
        crate::models::AppErrorKind::Cluster,
    )
}
