mod apply;
#[cfg(test)]
mod builtin_kind_read_tests;
mod details;
mod dynamic;
mod ingress_status;
mod kubeconform;
mod revisions;
mod scope;
mod summary;
mod summary_cluster;
mod summary_core;
mod summary_workloads;
pub(crate) mod topology;
#[cfg(feature = "bench-support")]
pub use topology::{build_resource_topology, input_from_metadata_at, TopologyInputResource};
mod topology_collection;
mod topology_dynamic;
mod topology_network;
#[cfg(test)]
mod topology_tests;
mod yaml;

#[cfg(feature = "bench-support")]
pub use apply::linting::lint_kubernetes_yaml_request;
#[cfg(feature = "bench-support")]
pub use apply::validation::validate_yaml_apply;
pub use apply::{apply_yaml, lint_kubernetes_yaml, prepare_yaml_apply};
#[cfg(feature = "bench-support")]
pub use details::{
    configmap_details_from_fetched, crd_details_from_fetched, deployment_details_from_fetched,
    secret_details_from_fetched,
};
pub use details::{get_resource_details, resource_details_from};
pub(crate) use dynamic::api_resource_from_discovered;
#[cfg(feature = "bench-support")]
pub use dynamic::dynamic_resource_details_from_object;
#[cfg(feature = "bench-support")]
pub use dynamic::dynamic_resource_summaries_at;
#[cfg(feature = "bench-support")]
pub use dynamic::serialize_dynamic_resource_document;
pub use dynamic::{
    dynamic_resource_details_from, dynamic_resources_summary_from, get_dynamic_resource_details,
    list_dynamic_resources,
};
#[cfg(feature = "bench-support")]
pub use kubeconform::collect_kubeconform_bytes;
#[cfg(feature = "bench-support")]
pub use revisions::deployment_revisions_from_replica_sets;
pub use revisions::list_deployment_revisions;
#[cfg(feature = "bench-support")]
pub use scope::resource_scope_fetch_plan;
#[cfg(feature = "bench-support")]
pub use scope::resource_scope_post_fetch;
pub use scope::{list_resource_scope, resource_scope_from};
pub use summary::{list_resources, resources_summary_from};
#[cfg(feature = "bench-support")]
pub use summary_cluster::{
    custom_resource_definition_resource_summary_at, node_resource_summary_at,
    persistent_volume_resource_summary_at, storage_class_resource_summary_at,
};
#[cfg(feature = "bench-support")]
pub use summary_core::{
    config_map_resource_summary_at, persistent_volume_claim_resource_summary_at,
    pod_resource_summary_at, secret_resource_summary_at, service_resource_summary_at,
};
#[cfg(feature = "bench-support")]
pub use summary_workloads::{
    cronjob_resource_summary_at, daemonset_resource_summary_at, deployment_resource_summary_at,
    ingress_resource_summary_at, job_resource_summary_at, replicaset_resource_summary_at,
    statefulset_resource_summary_at,
};
pub use topology::{list_resource_topology, resource_topology_from};
#[cfg(feature = "bench-support")]
pub use topology_collection::network_topology_from_objects_at;
#[cfg(feature = "bench-support")]
pub use topology_collection::topology_inputs_with_flux_ownership;
#[cfg(feature = "bench-support")]
pub use topology_collection::{inputs_from_metadata_at, pod_input_at};
#[cfg(feature = "bench-support")]
pub use topology_dynamic::dynamic_topology_input_at;
#[cfg(feature = "bench-support")]
pub use yaml::serialize_secret_resource_document;
pub use yaml::{get_resource_yaml, resource_yaml_from, reveal_secret_data_value};
