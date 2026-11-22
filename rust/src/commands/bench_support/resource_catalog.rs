mod cache;
mod incidents;
mod inspection;
mod planning;
mod resource_metrics;
mod summaries;
mod topology;

pub use cache::{ResourceCacheFixture, resource_cache_fixture};
pub use incidents::{
    IncidentFixture, ResourceEventFixture, incident_fixture, prepare_incident_summary,
    prepare_resource_events, resource_event_fixture,
};
pub use inspection::{
    EditorFixture, KubeconfigFixture, RevisionFixture, SchemaOutputFixture, YamlDocumentFixture,
    configmap_details, crd_details, deployment_details, deployment_revisions, dynamic_details,
    editor_apply_validation, editor_fixture, editor_local_lint, kubeconfig_context_projection,
    kubeconfig_source_fixture, revision_fixture, schema_output_fixture, schema_output_projection,
    secret_details, serialize_dynamic_document, serialize_secret_yaml, serialize_typed_document,
    yaml_document_fixture,
};
pub use planning::{
    FluxIndexFixture, FluxIndexProjection, ResourceScopeFixture, assemble_resource_scope,
    build_flux_index_fixture, flux_index_fixture, resource_scope_fixture, scope_fetch_plan,
};
pub use resource_metrics::{
    ResourceMetricsFixture, project_resource_metrics, resource_metrics_fixture,
};
pub use summaries::{
    DynamicSummaryFixture, ResourceSummaryFixture, crd_definition_fixture, crd_resource_catalog,
    dynamic_edge_fixtures, dynamic_edge_summaries_at, dynamic_summaries_at,
    dynamic_summary_fixture, namespace_summaries_at, namespace_summary_fixture,
    resource_summary_fixture, typed_summaries_at,
};
pub use topology::{
    NetworkTopologyFixture, OwnershipTopologyFixture, build_network_topology,
    build_ownership_topology, ownership_topology_fixture, topology_fixture,
};

use chrono::{DateTime, Utc};

pub fn fixed_now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-01T12:00:00Z")
        .expect("fixed benchmark time")
        .with_timezone(&Utc)
}
