pub mod commands;
pub mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::list_kube_contexts, commands::list_namespaces])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
