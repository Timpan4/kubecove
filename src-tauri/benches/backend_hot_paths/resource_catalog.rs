use codspeed_divan_compat::black_box;
use kubecove_lib::commands::bench_support::resource_catalog::*;
use kubecove_lib::models::{
    ResourceEventSummary, ResourceMetricsSummary, ResourceSummary, ResourceTopology, YamlEncoding,
    YamlViewMode,
};
use std::sync::LazyLock;

static SUMMARY_INPUT: LazyLock<ResourceSummaryFixture> =
    LazyLock::new(|| resource_summary_fixture(10_000));
static DYNAMIC_INPUT: LazyLock<DynamicSummaryFixture> =
    LazyLock::new(|| dynamic_summary_fixture(10_000));
static DYNAMIC_EDGE_INPUT: LazyLock<Vec<DynamicSummaryFixture>> =
    LazyLock::new(dynamic_edge_fixtures);
static NAMESPACES: LazyLock<Vec<k8s_openapi::api::core::v1::Namespace>> =
    LazyLock::new(|| namespace_summary_fixture(100));
static CRDS: LazyLock<Vec<k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition>> =
    LazyLock::new(|| crd_definition_fixture(1_000));
static KUBECONFIG_INPUT: LazyLock<KubeconfigFixture> = LazyLock::new(kubeconfig_source_fixture);
static FLUX_INPUT: LazyLock<FluxIndexFixture> = LazyLock::new(flux_index_fixture);
static SCOPE_INPUT: LazyLock<ResourceScopeFixture> = LazyLock::new(resource_scope_fixture);
static ALL_NAMESPACE_SCOPE_REQUESTS: LazyLock<Vec<kubecove_lib::models::ResourceListRequest>> =
    LazyLock::new(|| {
        vec![
            kubecove_lib::models::ResourceListRequest {
                kind: Some("Pod".to_string()),
                resource_kind: None,
                namespace: None,
            },
            kubecove_lib::models::ResourceListRequest {
                kind: Some("Pod".to_string()),
                resource_kind: None,
                namespace: Some("tenant-42".to_string()),
            },
        ]
    });
static OWNERSHIP_INPUT: LazyLock<OwnershipTopologyFixture> =
    LazyLock::new(ownership_topology_fixture);
static NETWORK_INPUT: LazyLock<NetworkTopologyFixture> = LazyLock::new(topology_fixture);
static DOCUMENT_INPUT: LazyLock<YamlDocumentFixture> = LazyLock::new(yaml_document_fixture);
static REVISION_INPUT: LazyLock<RevisionFixture> = LazyLock::new(revision_fixture);
static EDITOR_INPUT: LazyLock<EditorFixture> = LazyLock::new(editor_fixture);
static SCHEMA_INPUT: LazyLock<SchemaOutputFixture> = LazyLock::new(schema_output_fixture);
static INCIDENT_INPUT: LazyLock<IncidentFixture> = LazyLock::new(incident_fixture);
static EVENT_INPUT: LazyLock<ResourceEventFixture> = LazyLock::new(resource_event_fixture);
static METRICS_INPUT: LazyLock<ResourceMetricsFixture> = LazyLock::new(resource_metrics_fixture);

static NAMESPACE_OUTPUT: LazyLock<Vec<kubecove_lib::models::NamespaceSummary>> =
    LazyLock::new(|| namespace_summaries_at(&NAMESPACES, fixed_now()));
static CRD_CATALOG_OUTPUT: LazyLock<Vec<kubecove_lib::models::DiscoveredResourceKind>> =
    LazyLock::new(|| crd_resource_catalog(CRDS.clone()));
static SCOPE_OUTPUT: LazyLock<Vec<ResourceSummary>> =
    LazyLock::new(|| assemble_resource_scope(SCOPE_INPUT.clone()).expect("scope response fixture"));
static KUBECONFIG_OUTPUT: LazyLock<Vec<kubecove_lib::models::ClusterContext>> =
    LazyLock::new(|| kubeconfig_context_projection(&KUBECONFIG_INPUT).expect("context fixture"));
static DEPLOYMENT_DETAILS_OUTPUT: LazyLock<kubecove_lib::models::ResourceDetailsFull> =
    LazyLock::new(|| {
        deployment_details(DOCUMENT_INPUT.deployment.clone()).expect("details fixture")
    });
static CONFIGMAP_DETAILS_OUTPUT: LazyLock<kubecove_lib::models::ResourceDetailsFull> =
    LazyLock::new(|| {
        configmap_details(DOCUMENT_INPUT.config_map.clone()).expect("details fixture")
    });
static SECRET_DETAILS_OUTPUT: LazyLock<kubecove_lib::models::ResourceDetailsFull> =
    LazyLock::new(|| secret_details(DOCUMENT_INPUT.secret.clone()).expect("details fixture"));
static CRD_DETAILS_OUTPUT: LazyLock<kubecove_lib::models::ResourceDetailsFull> =
    LazyLock::new(|| crd_details(DOCUMENT_INPUT.crd.clone()).expect("details fixture"));
static DYNAMIC_DETAILS_OUTPUT: LazyLock<kubecove_lib::models::ResourceDetailsFull> =
    LazyLock::new(|| {
        dynamic_details(
            DOCUMENT_INPUT.dynamic_kind.clone(),
            DOCUMENT_INPUT.dynamic_object.clone(),
            YamlViewMode::ApplyClean,
            YamlEncoding::Kyaml,
        )
        .expect("details fixture")
    });
static REVISION_OUTPUT: LazyLock<Vec<kubecove_lib::models::DeploymentRevision>> =
    LazyLock::new(|| deployment_revisions(REVISION_INPUT.clone()));
static EDITOR_LINT_OUTPUT: LazyLock<kubecove_lib::models::KubernetesYamlLintResult> =
    LazyLock::new(|| editor_local_lint(EDITOR_INPUT.multiple_documents.clone()));

static TYPED_OUTPUT: LazyLock<Vec<ResourceSummary>> =
    LazyLock::new(|| typed_summaries_at(&SUMMARY_INPUT, fixed_now()));
static DYNAMIC_OUTPUT: LazyLock<Vec<ResourceSummary>> =
    LazyLock::new(|| dynamic_summaries_at(&DYNAMIC_INPUT, fixed_now()));
static DYNAMIC_EDGE_OUTPUT: LazyLock<Vec<ResourceSummary>> =
    LazyLock::new(|| dynamic_edge_summaries_at(&DYNAMIC_EDGE_INPUT, fixed_now()));
static OWNERSHIP_OUTPUT: LazyLock<ResourceTopology> =
    LazyLock::new(|| build_ownership_topology(OWNERSHIP_INPUT.clone()));
static NETWORK_OUTPUT: LazyLock<ResourceTopology> =
    LazyLock::new(|| build_network_topology(NETWORK_INPUT.clone()));
static METRICS_OUTPUT: LazyLock<ResourceMetricsSummary> =
    LazyLock::new(|| project_resource_metrics(&METRICS_INPUT));
static INCIDENT_OUTPUT: LazyLock<kubecove_lib::models::IncidentCockpitSummary> =
    LazyLock::new(|| prepare_incident_summary(INCIDENT_INPUT.clone()));
static EVENT_OUTPUT: LazyLock<Vec<ResourceEventSummary>> =
    LazyLock::new(|| prepare_resource_events(EVENT_INPUT.clone()));

#[path = "resource_catalog/cache.rs"]
mod cache;
#[path = "resource_catalog/encoding.rs"]
mod encoding;
#[path = "resource_catalog/incidents.rs"]
mod incidents;
#[path = "resource_catalog/inspection.rs"]
mod inspection;
#[path = "resource_catalog/planning.rs"]
mod planning;
#[path = "resource_catalog/resource_metrics.rs"]
mod resource_metrics;
#[path = "resource_catalog/summaries.rs"]
mod summaries;
#[path = "resource_catalog/topology.rs"]
mod topology;
