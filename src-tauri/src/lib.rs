pub mod commands;
pub mod models;

use commands::{
    detect_argocd, get_argocd_application_details, get_argocd_appproject_details,
    get_argocd_appset_details, get_dynamic_resource_details, get_resource_details,
    get_resource_yaml, list_argocd_applications, list_argocd_appprojects, list_argocd_appsets,
    list_dynamic_resources, list_kube_contexts, list_namespaces, list_resource_events,
    list_resource_kinds, list_resources,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_kube_contexts,
            list_namespaces,
            list_resource_kinds,
            list_resources,
            list_dynamic_resources,
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
            get_argocd_appproject_details
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
