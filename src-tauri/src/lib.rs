#![deny(clippy::all, clippy::pedantic, clippy::style)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::ref_option,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unused_async
)]

pub mod commands;
#[cfg(feature = "desktop-release")]
pub mod desktop_release;
#[cfg(feature = "mcp-host")]
pub mod mcp;
#[cfg(feature = "pinned-kubeconfig")]
pub use kubecove_kube::e2e;
pub use kubecove_kube::runtime;
pub use kubecove_models as models;
