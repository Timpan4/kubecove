mod apply;
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
mod topology_collection;
mod topology_dynamic;
mod topology_network;
#[cfg(test)]
mod topology_tests;
mod yaml;

pub use apply::{apply_yaml, lint_kubernetes_yaml, prepare_yaml_apply};
pub use details::{get_resource_details, resource_details_from};
pub(crate) use dynamic::{api_resource_from_discovered, dynamic_resource_summary};
pub use dynamic::{
    dynamic_resource_details_from, dynamic_resources_summary_from, get_dynamic_resource_details,
    list_dynamic_resources,
};
pub use revisions::list_deployment_revisions;
pub use scope::{list_resource_scope, resource_scope_from};
pub use summary::{list_resources, resources_summary_from};
pub use topology::{list_resource_topology, resource_topology_from};
pub use yaml::{get_resource_yaml, resource_yaml_from, reveal_secret_data_value};
