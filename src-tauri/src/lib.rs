pub mod commands;
pub mod models;

use commands::{
    apply_yaml, detect_argocd, get_app_usage_metrics, get_argocd_application_details,
    get_argocd_appproject_details, get_argocd_appset_details, get_dynamic_resource_details,
    get_helm_release_details, get_helm_release_reconciliation, get_resource_details,
    get_resource_yaml, lint_kubernetes_yaml, list_argocd_applications, list_argocd_appprojects,
    list_argocd_appsets, list_dynamic_resources, list_helm_releases, list_incident_cockpit,
    list_kube_contexts, list_namespaces, list_pod_exec_sessions, list_port_forwards,
    list_rbac_inspection, list_resource_events, list_resource_kinds, list_resource_metrics,
    list_resource_scope, list_resource_topology, list_resources, prepare_yaml_apply,
    resize_pod_exec_terminal, start_pod_exec_session, start_pod_log_stream, start_pod_port_forward,
    start_resource_event_watch, start_resource_watch, stop_pod_exec_session, stop_port_forward,
    stop_stream, write_pod_exec_stdin, AppUsageMonitor, ClusterLiveStore, PodExecRegistry,
    PortForwardRegistry, StreamRegistry,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(ClusterLiveStore::default())
        .manage(StreamRegistry::default())
        .manage(PortForwardRegistry::default())
        .manage(PodExecRegistry::default())
        .manage(AppUsageMonitor::default())
        .invoke_handler(tauri::generate_handler![
            list_kube_contexts,
            list_namespaces,
            list_resource_kinds,
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
            get_app_usage_metrics
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
