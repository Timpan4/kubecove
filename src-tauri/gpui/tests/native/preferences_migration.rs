//! Failure modes: the old session file loses presets, defaults drift between
//! controls, a corrupt new document silently falls back to older data, or a
//! version 1 kind name loses the API identity its import carried, takes
//! another document's kinds, or is dropped.
use kubecove_gpui::preferences::Store;
use kubecove_gpui::workspaces::{ApiKind, SavedKind};

#[test]
fn version_one_kind_names_take_their_import_identity() {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(".t")
        .join("preferences-tests")
        .join(format!("kinds-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let imported: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../src/lib/overview-application-workspace.json"
    ))
    .unwrap();
    let workspace = |id: u64, kinds: &[&str]| {
        serde_json::json!({
            "id": id, "name": format!("Saved {id}"), "context": "mock-dev", "group": [],
            "group_name": null, "namespaces": [], "kinds": kinds, "updated": id, "reviews": []
        })
    };
    let original = serde_json::json!({
        "version": 1,
        "workspaces": [
            workspace(1, &["Pod", "Application"]),
            workspace(2, &["Application"]),
            workspace(3, &["Pod", "Application"]),
        ],
        // Workspace 2's names differ from its document's kinds, so they stay.
        "workspaceDocuments": {"1": imported, "2": imported}
    })
    .to_string();
    let path = directory.join("live-sessions.json");
    std::fs::write(&path, &original).unwrap();
    let store = Store::file(directory.clone());
    let migrated = store.load().unwrap();
    assert_eq!(migrated.version, 2);
    let name = |name: &str| SavedKind::Name(name.into());
    let application = SavedKind::Api(ApiKind {
        group: "argoproj.io".into(),
        version: "v1alpha1".into(),
        api_version: "argoproj.io/v1alpha1".into(),
        kind: "Application".into(),
        plural: "applications".into(),
        namespaced: true,
    });
    let kinds: Vec<_> = migrated
        .workspaces
        .iter()
        .map(|saved| &saved.kinds)
        .collect();
    assert_eq!(
        kinds,
        [
            &vec![name("Pod"), application],
            &vec![name("Application")],
            &vec![name("Pod"), name("Application")],
        ]
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let saved = store.save(migrated).unwrap();
    assert_eq!(
        serde_json::to_value(Store::file(directory.clone()).load().unwrap()).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn session_document_migrates_without_replacing_the_original() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(".t")
        .join("preferences-tests")
        .join(format!("migration-{}", std::process::id()));
    let legacy = root.join("old/live-sessions.json");
    let directory = root.join("desktop");
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    let original = serde_json::json!({
        "presets": {"workspace": [{"label": "API", "request": {
            "clusterContext": "mock-dev", "namespace": "payments",
            "targetKind": "Service", "targetName": "payments-api", "remotePort": 80
        }}]},
        "autoStart": true,
        "keep": true
    })
    .to_string();
    std::fs::write(&legacy, &original).unwrap();
    let mut store = Store::file(directory.clone());
    store.set_legacy_file(legacy.clone());
    let migrated = store.load().unwrap();
    assert!(migrated.preferences.auto_start_saved_port_forwards);
    assert!(migrated.preferences.keep_live_sessions_on_workspace_switch);
    assert_eq!(migrated.saved_forward_presets["workspace"][0].label, "API");
    let saved = store.save(migrated).unwrap();
    assert_eq!(std::fs::read_to_string(&legacy).unwrap(), original);
    assert_eq!(
        serde_json::to_value(store.load().unwrap()).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    std::fs::write(directory.join("live-sessions.json"), "{").unwrap();
    assert!(store.load().is_err());
    assert!(store.save(saved).is_err());
    assert_eq!(std::fs::read_to_string(&legacy).unwrap(), original);
    std::fs::remove_dir_all(root).unwrap();
}
