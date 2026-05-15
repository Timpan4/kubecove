mod details;
mod summary;
mod yaml;

pub use details::{get_resource_details, resource_details_from};
pub use summary::{list_resources, resources_summary_from};
pub use yaml::{get_resource_yaml, resource_yaml_from};
