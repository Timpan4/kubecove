//! Failure modes: restart loses edits; migration changes safety defaults; corrupt
//! or future data is overwritten; failed writes publish a false success; stale
//! instances overwrite newer edits; credentials enter a preference document.
use kubecove_gpui::preferences::{Document, Store};
use std::path::PathBuf;

fn directory() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(".t")
        .join("preferences-tests");
    let path = root.join(format!(
        "preferences-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn restart_and_stale_writes() {
    let directory = directory();
    let store = Store::file(directory.clone());
    let mut document = store.load().unwrap();
    document.preferences.show_exact_timestamps = true;
    document.preferences.log_layout = kubecove_gpui::preferences::LogLayout::Gutter;
    let mut values = serde_json::to_value(&document.preferences).unwrap();
    assert_eq!(values.as_object().unwrap().len(), 24);
    for value in values.as_object_mut().unwrap().values_mut() {
        if let Some(enabled) = value.as_bool() {
            *value = serde_json::json!(!enabled);
        }
    }
    values["timestampTimezone"] = serde_json::json!("utc");
    values["yamlViewModeDefault"] = serde_json::json!("applyClean");
    values["yamlEncodingDefault"] = serde_json::json!("kyaml");
    values["yamlDiffStyle"] = serde_json::json!("git");
    values["gitOpsViewMode"] = serde_json::json!("list");
    values["helmViewMode"] = serde_json::json!("list");
    values["resourceKindLanes"] =
        serde_json::json!({"first":["Secret"],"normal":["*"],"last":["Pod"]});
    values["argoProfiles"] = serde_json::json!([{"id":"argo","endpoint":{"kind":"externalHttps","url":"https://argo.example.test"},"clusterContext":"mock-dev","workspaceId":"1","kubeconfigSourceKey":"source","rememberCredential":true}]);
    values["argoConnectionPreferences"] =
        serde_json::json!({"1":{"kind":"connected","profileId":"argo"}});
    document.preferences = serde_json::from_value(values).unwrap();
    document.updates.last_checked_at = Some("2026-11-05T00:00:00Z".into());
    document.updates.dismissed_version = Some("1.0.0".into());
    let mut workspace = kubecove_gpui::workspaces::Draft {
        name: "Private review",
        effective: "mock-dev",
        group: &[],
        namespaces: &[],
        kinds: None,
    }
    .build(1, 1);
    let record = serde_json::json!({"cluster_context":"mock-dev","object_key":"Role/payments/reader","evidence_fingerprint":"rbac-v1-test","disposition":"expected","note":"Approved for this workspace","reviewed_at":"2026-11-05T00:00:00Z"});
    workspace.reviews = serde_json::from_value(serde_json::json!([record])).unwrap();
    document.workspaces.push(workspace);
    let saved = store.save(document).unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "preferences::reload_child", "--nocapture"])
        .env("KUBECOVE_PREFERENCES_TEST_DIRECTORY", &directory)
        .status()
        .unwrap();
    assert!(child.success());
    let reopened = Store::file(directory.clone());
    assert_eq!(
        serde_json::to_value(reopened.load().unwrap()).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    let mut stale = saved.clone();
    stale.preferences.redact_secrets = false;
    store.save(saved).unwrap();
    assert!(reopened.save(stale).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn reload_child() {
    let Some(directory) = std::env::var_os("KUBECOVE_PREFERENCES_TEST_DIRECTORY") else {
        return;
    };
    let document = Store::file(directory.into()).load().unwrap();
    assert_eq!(
        document.workspaces[0].reviews[0].note,
        "Approved for this workspace"
    );
    assert_eq!(document.preferences.argo_profiles[0].id, "argo");
    assert_eq!(document.updates.dismissed_version.as_deref(), Some("1.0.0"));
}

#[test]
fn corrupt_future_migration_and_write_failure() {
    let directory = directory();
    let path = directory.join("live-sessions.json");
    for raw in [
        "{",
        r#"{"version":99}"#,
        r#"{"version":1,"preferences":{"token":"forbidden"}}"#,
    ] {
        std::fs::write(&path, raw).unwrap();
        let store = Store::file(directory.clone());
        assert!(store.load().is_err());
        assert!(store.save(Document::default()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);
    }
    std::fs::write(
        &path,
        r#"{"version":0,"preferences":{"showExactTimestamps":true}}"#,
    )
    .unwrap();
    let store = Store::file(directory.clone());
    let migrated = store.load().unwrap();
    assert!(migrated.preferences.show_exact_timestamps);
    assert!(migrated.preferences.redact_secrets);
    assert_eq!(migrated.version, 1);
    store.save(migrated).unwrap();
    let original = std::fs::read(&path).unwrap();
    // A directory in place of the sibling lock file forces a write error,
    // including when tests run as a privileged user.
    std::fs::remove_file(directory.join("live-sessions.lock")).unwrap();
    std::fs::create_dir(directory.join("live-sessions.lock")).unwrap();
    assert!(store.save(store.load().unwrap()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_dir_all(directory).unwrap();
}
