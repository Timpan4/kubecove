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

mod appimage_webkit;
pub mod commands;
#[cfg(feature = "e2e")]
use kubecove_kube::e2e;
pub use kubecove_kube::runtime;
pub use kubecove_models as models;

#[cfg(feature = "desktop")]
use commands::{
    cancel_workspace_requests, init_kubeconfig_settings_path, AppUsageMonitor, ArgoConnectionStore,
    BackendCancellationRegistry, ClusterLiveStore, PodExecRegistry, PortForwardRegistry,
    RemoteConnectionStore, StreamRegistry,
};
#[cfg(all(feature = "desktop", not(feature = "e2e")))]
use tauri::Manager;
#[cfg(feature = "desktop")]
use {
    kubecove_gitops as gitops, kubecove_host as host, kubecove_inspection as inspection,
    kubecove_kube as backend, kubecove_live as live,
};

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    appimage_webkit::prefer_newer_system_webkit();

    #[cfg(feature = "e2e")]
    let e2e_config = e2e::startup_config().expect("invalid KubeCove E2E environment");
    #[cfg(not(feature = "e2e"))]
    assert_ne!(
        std::env::var("KUBECOVE_E2E").as_deref(),
        Ok("1"),
        "KUBECOVE_E2E=1 requires the e2e Cargo feature"
    );

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(feature = "e2e")]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    builder
        .setup(move |app| {
            #[cfg(feature = "e2e")]
            {
                let _ = app;
                init_kubeconfig_settings_path(e2e_config.data_dir);
            }
            #[cfg(not(feature = "e2e"))]
            {
                #[cfg(debug_assertions)]
                let app_config_dir = if std::env::var("KUBECOVE_DEV_KIND").as_deref() == Ok("1") {
                    let path = std::env::var_os("KUBECOVE_DATA_DIR")
                        .map(std::path::PathBuf::from)
                        .ok_or("KUBECOVE_DATA_DIR is required for dev:kind")?;
                    if !path.is_absolute() || !path.is_dir() {
                        return Err(
                            "KUBECOVE_DATA_DIR must be an existing absolute directory".into()
                        );
                    }
                    path
                } else {
                    app.path().app_config_dir()?
                };
                #[cfg(not(debug_assertions))]
                let app_config_dir = app.path().app_config_dir()?;
                init_kubeconfig_settings_path(app_config_dir);
            }
            #[cfg(target_os = "linux")]
            {
                use std::ffi::{c_char, c_void, CStr};
                use tauri::Manager;
                use webkit2gtk::{glib::translate::ToGlibPtr, WebViewExt};

                if let Some(window) = app.get_webview_window("main") {
                    // WebKit otherwise renders near 60 fps on high-refresh monitors.
                    let _ =
                        window.with_webview(|webview| {
                            let Some(settings) = webview.inner().settings() else {
                                return;
                            };
                            // SAFETY: RTLD_DEFAULT searches loaded libraries; all symbol names
                            // are static NUL-terminated strings. Older WebKit returns null.
                            let symbols = unsafe {
                                [
                                    libc::dlsym(
                                        libc::RTLD_DEFAULT,
                                        c"webkit_settings_get_all_features".as_ptr(),
                                    ),
                                    libc::dlsym(
                                        libc::RTLD_DEFAULT,
                                        c"webkit_feature_list_get_length".as_ptr(),
                                    ),
                                    libc::dlsym(
                                        libc::RTLD_DEFAULT,
                                        c"webkit_feature_list_get".as_ptr(),
                                    ),
                                    libc::dlsym(
                                        libc::RTLD_DEFAULT,
                                        c"webkit_feature_get_identifier".as_ptr(),
                                    ),
                                    libc::dlsym(
                                        libc::RTLD_DEFAULT,
                                        c"webkit_settings_set_feature_enabled".as_ptr(),
                                    ),
                                    libc::dlsym(
                                        libc::RTLD_DEFAULT,
                                        c"webkit_feature_list_unref".as_ptr(),
                                    ),
                                ]
                            };
                            if symbols.iter().any(|symbol| symbol.is_null()) {
                                return;
                            }
                            // SAFETY: all six symbols are non-null WebKit C functions with
                            // these ABI signatures, and the loaded library stays resident.
                            let (
                                get_all_features,
                                get_length,
                                get_feature,
                                get_identifier,
                                set_enabled,
                                unref,
                            ) = unsafe {
                                (
                                    std::mem::transmute::<
                                        *mut c_void,
                                        unsafe extern "C" fn() -> *mut c_void,
                                    >(symbols[0]),
                                    std::mem::transmute::<
                                        *mut c_void,
                                        unsafe extern "C" fn(*mut c_void) -> usize,
                                    >(symbols[1]),
                                    std::mem::transmute::<
                                        *mut c_void,
                                        unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void,
                                    >(symbols[2]),
                                    std::mem::transmute::<
                                        *mut c_void,
                                        unsafe extern "C" fn(*mut c_void) -> *const c_char,
                                    >(symbols[3]),
                                    std::mem::transmute::<
                                        *mut c_void,
                                        unsafe extern "C" fn(
                                            *mut webkit2gtk::ffi::WebKitSettings,
                                            *mut c_void,
                                            webkit2gtk::glib::ffi::gboolean,
                                        ),
                                    >(symbols[4]),
                                    std::mem::transmute::<
                                        *mut c_void,
                                        unsafe extern "C" fn(*mut c_void),
                                    >(symbols[5]),
                                )
                            };
                            // SAFETY: with_webview runs on the GTK thread; settings stays alive.
                            // The owned list keeps its borrowed features and NUL-terminated identifiers
                            // alive until unref; null pointers are checked before use.
                            unsafe {
                                let list = get_all_features();
                                if list.is_null() {
                                    return;
                                }
                                for index in 0..get_length(list) {
                                    let feature = get_feature(list, index);
                                    if feature.is_null() {
                                        continue;
                                    }
                                    let identifier = get_identifier(feature);
                                    if !identifier.is_null()
                                        && CStr::from_ptr(identifier)
                                            == c"PreferPageRenderingUpdatesNear60FPS"
                                    {
                                        set_enabled(settings.to_glib_none().0, feature, 0);
                                        break;
                                    }
                                }
                                unref(list);
                            }
                        });
                }
            }
            Ok(())
        })
        .manage(ClusterLiveStore::default())
        .manage(BackendCancellationRegistry::default())
        .manage(StreamRegistry::default())
        .manage(PortForwardRegistry::default())
        .manage(PodExecRegistry::default())
        .manage(AppUsageMonitor::default())
        .manage(ArgoConnectionStore::default())
        .manage(RemoteConnectionStore::default())
        .invoke_handler(tauri::generate_handler![
            backend::list_kube_contexts,
            inspection::list_namespaces,
            inspection::list_resource_kinds,
            inspection::list_present_custom_resource_kinds,
            inspection::list_resources,
            inspection::list_deployment_revisions,
            inspection::list_dynamic_resources,
            inspection::list_resource_scope,
            inspection::list_resource_topology,
            inspection::list_resource_metrics,
            inspection::list_resource_events,
            inspection::get_resource_yaml,
            inspection::reveal_secret_data_value,
            inspection::get_resource_details,
            inspection::get_dynamic_resource_details,
            inspection::prepare_yaml_apply,
            inspection::apply_yaml,
            inspection::lint_kubernetes_yaml,
            inspection::get_resource_action_state,
            inspection::preview_resource_action,
            inspection::execute_resource_action,
            inspection::preview_scale_workload,
            inspection::scale_workload,
            inspection::preview_rollout_restart,
            inspection::rollout_restart,
            inspection::preview_delete_resource,
            inspection::delete_resource,
            gitops::detect_argocd,
            gitops::list_argocd_applications,
            gitops::get_argocd_application_details,
            gitops::list_argocd_appsets,
            gitops::list_argocd_appprojects,
            gitops::get_argocd_appset_details,
            gitops::get_argocd_appproject_details,
            gitops::discover_argo_servers,
            gitops::connect_argo_server,
            gitops::get_argo_connection_status,
            gitops::disconnect_argo_server,
            gitops::forget_argo_credential,
            gitops::get_argo_application_inspector,
            gitops::get_argo_resource_comparison,
            gitops::preflight_argo_operation,
            gitops::run_argo_operation,
            gitops::detect_flux,
            gitops::list_flux_resources,
            gitops::get_flux_resource_details,
            gitops::list_helm_releases,
            gitops::get_helm_release_details,
            gitops::get_helm_release_reconciliation,
            inspection::list_incident_cockpit,
            inspection::list_rbac_inspection,
            inspection::review_rbac_access,
            live::start_resource_watch,
            live::refresh_resource_cache,
            live::start_resource_event_watch,
            live::start_pod_log_stream,
            live::start_aggregated_log_stream,
            live::stop_stream,
            live::start_pod_port_forward,
            live::stop_port_forward,
            live::list_port_forwards,
            live::start_pod_exec_session,
            live::write_pod_exec_stdin,
            live::resize_pod_exec_terminal,
            live::stop_pod_exec_session,
            live::list_pod_exec_sessions,
            live::stop_live_sessions_outside_scope,
            host::usage::get_app_usage_metrics,
            backend::get_kubeconfig_sources,
            backend::set_kubeconfig_env_var,
            backend::set_show_kubeconfig_source_labels,
            backend::pick_kubeconfig_paths,
            host::workspace_files::save_workspace_export_json,
            host::workspace_files::pick_workspace_import_json,
            backend::add_kubeconfig_paths,
            backend::remove_kubeconfig_path,
            backend::reorder_kubeconfig_paths,
            backend::set_backend_diagnostics_enabled,
            backend::get_backend_diagnostics,
            backend::get_backend_cache_diagnostics,
            backend::clear_backend_diagnostics,
            backend::cancel_backend_requests,
            cancel_workspace_requests,
            host::remote::remote_connect,
            host::remote::remote_disconnect,
            host::remote::remote_connection_status,
            host::remote::remote_invoke,
            host::remote::remote_subscribe_channel,
            host::remote::remote_unsubscribe_channel
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
