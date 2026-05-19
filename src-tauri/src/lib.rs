pub mod commands;
pub mod models;

use commands::{
    detect_argocd, get_argocd_application_details, get_argocd_appproject_details,
    get_argocd_appset_details, get_dynamic_resource_details, get_resource_details,
    get_resource_yaml, list_argocd_applications, list_argocd_appprojects, list_argocd_appsets,
    list_dynamic_resources, list_kube_contexts, list_namespaces, list_resource_events,
    list_resource_kinds, list_resource_scope, list_resource_topology, list_resources,
    start_pod_log_stream, start_resource_event_watch, start_resource_watch, stop_stream,
    ClusterLiveStore, StreamRegistry,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ClusterLiveStore::default())
        .manage(StreamRegistry::default())
        .invoke_handler(tauri::generate_handler![
            list_kube_contexts,
            list_namespaces,
            list_resource_kinds,
            list_resources,
            list_dynamic_resources,
            list_resource_scope,
            list_resource_topology,
            list_resource_events,
            get_resource_yaml,
            get_resource_details,
            get_dynamic_resource_details,
            detect_argocd,
            list_argocd_applications,
            get_argocd_application_details,
            list_argocd_appsets,
            list_argocd_appprojects,
            get_argocd_appset_details,
            get_argocd_appproject_details,
            start_resource_watch,
            start_resource_event_watch,
            start_pod_log_stream,
            stop_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
