pub mod commands;
pub mod models;

use commands::{list_kube_contexts, list_namespaces, list_resources};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_kube_contexts, list_namespaces, list_resources])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
