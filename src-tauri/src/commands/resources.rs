mod details;
mod dynamic;
mod summary;
mod summary_cluster;
mod summary_core;
mod summary_workloads;
mod yaml;

pub use details::{get_resource_details, resource_details_from};
pub use dynamic::{
    dynamic_resource_details_from, dynamic_resources_summary_from, get_dynamic_resource_details,
    list_dynamic_resources,
};
pub use summary::{list_resources, resources_summary_from};
pub use yaml::{get_resource_yaml, resource_yaml_from};
