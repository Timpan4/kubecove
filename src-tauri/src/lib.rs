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
pub mod models;

use commands::{
    add_kubeconfig_paths, apply_yaml, cancel_backend_requests, clear_backend_diagnostics,
    detect_argocd, detect_flux, get_app_usage_metrics, get_argocd_application_details,
    get_argocd_appproject_details, get_argocd_appset_details, get_backend_diagnostics,
    get_dynamic_resource_details, get_flux_resource_details, get_helm_release_details,
    get_helm_release_reconciliation, get_kubeconfig_sources, get_resource_details,
    get_resource_yaml, init_kubeconfig_settings_path, lint_kubernetes_yaml,
    list_argocd_applications, list_argocd_appprojects, list_argocd_appsets, list_dynamic_resources,
    list_flux_resources, list_helm_releases, list_incident_cockpit, list_kube_contexts,
    list_namespaces, list_pod_exec_sessions, list_port_forwards,
    list_present_custom_resource_kinds, list_rbac_inspection, list_resource_events,
    list_resource_kinds, list_resource_metrics, list_resource_scope, list_resource_topology,
    list_resources, pick_kubeconfig_paths, prepare_yaml_apply, remove_kubeconfig_path,
    reorder_kubeconfig_paths, resize_pod_exec_terminal, set_backend_diagnostics_enabled,
    set_kubeconfig_env_var, set_show_kubeconfig_source_labels, start_pod_exec_session,
    start_pod_log_stream, start_pod_port_forward, start_resource_event_watch, start_resource_watch,
    stop_live_sessions_outside_scope, stop_pod_exec_session, stop_port_forward, stop_stream,
    write_pod_exec_stdin, AppUsageMonitor, BackendCancellationRegistry, ClusterLiveStore,
    PodExecRegistry, PortForwardRegistry, StreamRegistry,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            init_kubeconfig_settings_path(app.path().app_config_dir()?);
            Ok(())
        })
        .manage(ClusterLiveStore::default())
        .manage(BackendCancellationRegistry::default())
        .manage(StreamRegistry::default())
        .manage(PortForwardRegistry::default())
        .manage(PodExecRegistry::default())
        .manage(AppUsageMonitor::default())
        .invoke_handler(tauri::generate_handler![
            list_kube_contexts,
            list_namespaces,
            list_resource_kinds,
            list_present_custom_resource_kinds,
            list_resources,
            list_dynamic_resources,
            list_resource_scope,
            list_resource_topology,
            list_resource_metrics,
            list_resource_events,
            get_resource_yaml,
            get_resource_details,
            get_dynamic_resource_details,
            prepare_yaml_apply,
            apply_yaml,
            lint_kubernetes_yaml,
            detect_argocd,
            list_argocd_applications,
            get_argocd_application_details,
            list_argocd_appsets,
            list_argocd_appprojects,
            get_argocd_appset_details,
            get_argocd_appproject_details,
            detect_flux,
            list_flux_resources,
            get_flux_resource_details,
            list_helm_releases,
            get_helm_release_details,
            get_helm_release_reconciliation,
            list_incident_cockpit,
            list_rbac_inspection,
            start_resource_watch,
            start_resource_event_watch,
            start_pod_log_stream,
            stop_stream,
            start_pod_port_forward,
            stop_port_forward,
            list_port_forwards,
            start_pod_exec_session,
            write_pod_exec_stdin,
            resize_pod_exec_terminal,
            stop_pod_exec_session,
            list_pod_exec_sessions,
            stop_live_sessions_outside_scope,
            get_app_usage_metrics,
            get_kubeconfig_sources,
            set_kubeconfig_env_var,
            set_show_kubeconfig_source_labels,
            pick_kubeconfig_paths,
            add_kubeconfig_paths,
            remove_kubeconfig_path,
            reorder_kubeconfig_paths,
            set_backend_diagnostics_enabled,
            get_backend_diagnostics,
            clear_backend_diagnostics,
            cancel_backend_requests
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
