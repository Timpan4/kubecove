mod actions;
pub mod assets;
pub mod backend;
mod collate;
pub mod gitops;
mod incident;
mod incident_timeline;
mod incidents;
pub mod inspector;
mod live_sessions;
mod log_buffer;
mod logs;
pub mod model;
mod palette;
mod rbac;
mod settings;
pub mod theme;
mod topology;
mod ui;
mod unified_diff;
mod workspaces;

pub use inspector::Inspector;

/// Loads fonts and applies the KubeCove theme. Call after `gpui_kit::init`.
pub fn init(cx: &mut gpui_kit::App) -> gpui_kit::Result<()> {
    assets::load_fonts(cx)?;
    theme::apply(cx);
    Ok(())
}
