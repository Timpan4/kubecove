//! Storage is process-global in the shared backend. Isolate each failure mode
//! in a subprocess with a worktree-local directory and no user kubeconfig.
use kubecove_gpui::backend::{
    Backend, NativeBackend,
    settings_integrations::{UpdateStatus, update_status},
};
use std::{path::PathBuf, process::Command};

#[test]
fn storage_survives_restart_and_reports_failures() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.t");
    std::fs::create_dir_all(&root).unwrap();
    for case in ["persist", "reopen", "corrupt", "write-error"] {
        let directory = root.join(format!("native-settings-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        if case == "corrupt" {
            std::fs::write(directory.join("kubeconfig-sources.json"), "{").unwrap();
        }
        if case == "write-error" {
            std::fs::remove_file(directory.join("kubeconfig-sources.json")).unwrap();
            std::fs::create_dir(directory.join("kubeconfig-sources.json")).unwrap();
        }
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "settings_integrations::storage_child",
                "--nocapture",
            ])
            .env("KUBECOVE_SETTINGS_CASE", case)
            .env("KUBECOVE_SETTINGS_DIRECTORY", &directory)
            .env("HOME", &directory)
            .env("USERPROFILE", &directory)
            .env("KUBECONFIG", directory.join("missing-kubeconfig"))
            .status()
            .unwrap();
        assert!(result.success(), "storage case {case} failed");
        if case == "write-error" {
            std::fs::remove_dir_all(directory).unwrap();
        }
    }
}

#[test]
fn storage_child() {
    let Ok(case) = std::env::var("KUBECOVE_SETTINGS_CASE") else {
        return;
    };
    let directory = PathBuf::from(std::env::var_os("KUBECOVE_SETTINGS_DIRECTORY").unwrap());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = NativeBackend::with_settings_directory(directory.clone()).unwrap();
    match case.as_str() {
        "persist" => {
            runtime
                .block_on(backend.set_kubeconfig_env_var("PARITY_SOURCE".into()))
                .unwrap();
            runtime
                .block_on(backend.set_show_kubeconfig_source_labels(false))
                .unwrap();
        }
        "reopen" => {
            let sources = runtime.block_on(backend.kubeconfig_sources()).unwrap();
            assert_eq!(sources.kubeconfig_env_var, "PARITY_SOURCE");
            assert!(!sources.show_source_labels);
        }
        "corrupt" => assert!(runtime.block_on(backend.kubeconfig_sources()).is_err()),
        "write-error" => {
            // Warm the valid in-memory state, then make the storage path unwritable.
            std::fs::remove_dir(directory.join("kubeconfig-sources.json")).unwrap();
            runtime.block_on(backend.kubeconfig_sources()).unwrap();
            std::fs::create_dir(directory.join("kubeconfig-sources.json")).unwrap();
            assert!(
                runtime
                    .block_on(backend.set_kubeconfig_env_var("PARITY_SOURCE".into()))
                    .is_err()
            );
        }
        _ => panic!("unknown storage case"),
    }
}

#[test]
fn update_manifest_does_not_offer_current_or_invalid_versions() {
    for version in [env!("CARGO_PKG_VERSION"), "0.0.1"] {
        assert!(matches!(
            update_status(&serde_json::json!({"version":version})).unwrap(),
            UpdateStatus::UpToDate
        ));
    }
    for manifest in [
        serde_json::json!({}),
        serde_json::json!({"version":"invalid"}),
    ] {
        assert!(update_status(&manifest).is_err());
    }
}

#[test]
fn workspace_files_validate_before_the_import_preview() {
    use kubecove_gpui::workspaces::transfer::parse;
    for invalid in ["{", "{}", r#"{"apiVersion":"other","kind":"Workspace"}"#] {
        assert!(parse(invalid).is_err());
    }
    let document = serde_json::json!({"apiVersion":"kubecove.dev/workspace/v1","kind":"Workspace","metadata":{"name":"controlled"},"spec":{"displayName":"Controlled import","scope":{"clusterContext":"mock-dev","namespaces":["payments"],"kinds":["Pod"],"layout":"overview"},"shortcuts":[],"portForwards":[]}});
    let imported = parse(&document.to_string()).unwrap();
    assert_eq!(imported[0].context, "mock-dev");
    assert_eq!(imported[0].document, document);
    let mut invalid = document;
    invalid["spec"]["scope"]["clusterContext"] = serde_json::json!(false);
    assert!(parse(&invalid.to_string()).is_err());
}
