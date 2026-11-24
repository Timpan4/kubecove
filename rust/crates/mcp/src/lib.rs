//! Read-only MCP tools over the backend's inspection reads and their Streamable HTTP endpoint
//! (ADR 0028). A host supplies its cluster live store and session scope; every tool reads
//! through the Read cluster-access intent and none can change the cluster.

mod compact;
mod handler;
mod http;
mod kinds;
#[cfg(feature = "loopback")]
mod loopback;
mod token;
mod tools;

pub use handler::{McpContext, ToolCall, ToolObserver, ToolOutcome};
pub use http::{EndpointPolicy, McpEndpoint, McpResponse};
#[cfg(feature = "loopback")]
pub use loopback::serve_loopback;
pub use token::{TOKEN_FILE_NAME, load_or_create_token, new_token, rotate_token, write_token_file};
pub use tools::TOOL_NAMES;

#[cfg(test)]
mod tests;

#[cfg(feature = "loopback")]
#[doc(hidden)]
pub mod bench_support {
    pub use crate::tools::{describe_resource_body, page_resource_rows};
}
