pub mod commands;
pub mod models;

use commands::{detect_argocd, get_argocd_application_details, get_resource_details, get_resource_yaml, list_argocd_applications, list_argocd_appsets, list_argocd_appprojects, list_kube_contexts, list_namespaces, list_resources};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_kube_contexts,
            list_namespaces,
            list_resources,
            get_resource_yaml,
            get_resource_details,
            detect_argocd,
            list_argocd_applications,
            get_argocd_application_details,
            list_argocd_appsets,
            list_argocd_appprojects
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
