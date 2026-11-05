mod actions;
mod argo_app;
pub mod assets;
pub mod backend;
mod collate;
pub mod gitops;
mod gitops_surface;
mod helm;
mod incident;
mod incident_timeline;
mod incidents;
pub mod inspector;
mod live_sessions;
mod log_buffer;
mod logs;
mod metrics;
pub mod model;
mod palette;
pub mod preferences;
mod rbac;
mod read_retry;
mod resource_search;
mod settings;
mod table_sort;
pub mod theme;
mod topology;
mod ui;
mod unified_diff;
pub mod workspaces;

pub use inspector::Inspector;

/// Loads fonts and applies the KubeCove theme. Call after `gpui_kit::init`.
pub fn init(cx: &mut gpui_kit::App) -> gpui_kit::Result<()> {
    assets::load_fonts(cx)?;
    theme::apply(cx);
    Ok(())
}
pub mod diagnostics;
pub mod secret_values;
