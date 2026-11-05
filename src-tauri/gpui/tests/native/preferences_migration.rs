//! Failure modes: the old session file loses presets, defaults drift between
//! controls, or a corrupt new document silently falls back to older data.
use kubecove_gpui::preferences::Store;

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
