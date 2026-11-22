//! Failure modes across starts: a snapshot re-merges after a failed archive or
//! a Tauri republish and restores deleted entries; a later export is archived
//! unread; a stale diagnostic outlives a successful retry; presets are filed
//! under no kubeconfig source, or block the rest of the import while waiting
//! for one; shortcuts vanish when a workspace merges into a GPUI one.

use crate::scratch::Scratch;
use crate::tauri_migration::{fixed_source, scoped_preset_key, unresolved_source};
use kubecove_gpui::preferences::Store;
use serde_json::json;

fn snapshot_at(exported_at: &str, workspaces: serde_json::Value) -> String {
    json!({
        "schemaVersion": 1,
        "exportedAt": exported_at,
        "entries": {
            "kubecove-settings": { "version": 0, "state": { "showExactTimestamps": true } },
            "kubecove-workspaces": { "version": 0, "state": { "workspaces": workspaces } }
        }
    })
    .to_string()
}

fn named(names: &[&str]) -> serde_json::Value {
    names
        .iter()
        .map(|name| json!({ "id": name, "name": name, "scope": { "clusterContext": "dev" } }))
        .collect()
}

#[test]
fn tauri_migration_marker_blocks_reimport_after_failed_archive_or_republish() {
    let root = Scratch::new("preferences-tests", "tauri-marker");
    let directory = root.to_path_buf();
    let migration_path = directory.join("tauri-migration.json");
    let imported_path = directory.join("tauri-migration.imported.json");
    let snapshot = snapshot_at("2026-11-21T00:00:00Z", named(&["Alpha", "Beta"]));
    std::fs::write(&migration_path, &snapshot).unwrap();
    // A directory at the archive path makes archival fail after the merge is saved.
    std::fs::create_dir(&imported_path).unwrap();

    let store = Store::file(directory.clone());
    let imported = store.load().unwrap();
    assert_eq!(imported.workspaces.len(), 2);
    assert_eq!(
        imported.tauri_migration_imported.as_deref(),
        Some("2026-11-21T00:00:00Z")
    );
    assert!(store.last_migration_diagnostic().is_some());
    assert!(migration_path.exists());

    // The user deletes a migrated workspace; Tauri republishes the same snapshot.
    let mut edited = imported;
    edited.workspaces.retain(|w| w.name != "Beta");
    let saved = store.save(edited).unwrap();
    std::fs::write(&migration_path, &snapshot).unwrap();

    let reloaded = store.load().unwrap();
    assert_eq!(reloaded.revision, saved.revision);
    assert_eq!(reloaded.workspaces.len(), 1);
    assert_eq!(reloaded.workspaces[0].name, "Alpha");
    assert!(store.last_migration_diagnostic().is_some());

    // Once the archive path is free, the same store archives the snapshot and
    // drops the earlier failure.
    std::fs::remove_dir(&imported_path).unwrap();
    let archived = store.load().unwrap();
    assert_eq!(archived.revision, saved.revision);
    assert_eq!(store.last_migration_diagnostic(), None);
    assert!(!migration_path.exists());
    assert_eq!(std::fs::read_to_string(&imported_path).unwrap(), snapshot);

    // A later Tauri export is a different snapshot: it is neither merged nor
    // archived over the imported one, and the user is told why.
    let newer = snapshot_at("2026-11-22T00:00:00Z", named(&["Alpha", "Beta", "Gamma"]));
    std::fs::write(&migration_path, &newer).unwrap();
    let unchanged = store.load().unwrap();
    assert_eq!(unchanged.revision, saved.revision);
    assert_eq!(unchanged.workspaces.len(), 1);
    assert!(
        store
            .last_migration_diagnostic()
            .unwrap()
            .contains("already imported the snapshot exported at 2026-11-21T00:00:00Z")
    );
    assert_eq!(std::fs::read_to_string(&migration_path).unwrap(), newer);
    assert_eq!(std::fs::read_to_string(&imported_path).unwrap(), snapshot);
}

#[test]
fn tauri_migration_imports_everything_but_presets_until_the_source_resolves() {
    let root = Scratch::new("preferences-tests", "tauri-source");
    let directory = root.to_path_buf();
    let migration_path = directory.join("tauri-migration.json");
    let forward = json!({
        "clusterContext": "dev",
        "namespace": "web",
        "serviceName": "api",
        "servicePort": 80
    });
    let snapshot = snapshot_at(
        "2026-11-21T00:00:00Z",
        json!([
            { "id": "a", "name": "Alpha", "scope": { "clusterContext": "dev" }, "portForwards": [forward] },
            { "id": "b", "name": "Beta", "scope": { "clusterContext": "dev" }, "portForwards": [forward] }
        ]),
    );
    std::fs::write(&migration_path, &snapshot).unwrap();

    let waiting = Store::file(directory.clone()).with_source_resolver(unresolved_source);
    let first = waiting.load().unwrap();
    assert_eq!(first.workspaces.len(), 2);
    assert!(first.preferences.show_exact_timestamps);
    assert!(first.tauri_migration_presets_pending);
    assert!(first.saved_forward_presets.is_empty());
    assert!(
        waiting
            .last_migration_diagnostic()
            .unwrap()
            .contains("kubeconfig settings unreadable")
    );
    assert_eq!(std::fs::read_to_string(&migration_path).unwrap(), snapshot);

    // Saves in between keep the user's choices: the imported settings were
    // already committed, and a deleted workspace gets no presets later.
    let mut edited = first;
    edited.preferences.show_exact_timestamps = false;
    edited.workspaces.retain(|w| w.name != "Beta");
    let saved = waiting.save(edited).unwrap();
    assert_eq!(waiting.load().unwrap().revision, saved.revision);
    assert!(migration_path.exists());

    let resolved = Store::file(directory.clone()).with_source_resolver(fixed_source);
    let finished = resolved.load().unwrap();
    assert!(!finished.tauri_migration_presets_pending);
    assert!(!finished.preferences.show_exact_timestamps);
    assert_eq!(finished.workspaces.len(), 1);
    assert_eq!(finished.saved_forward_presets.len(), 1);
    assert_eq!(
        finished.saved_forward_presets[&scoped_preset_key("dev", "Alpha")].len(),
        1
    );
    assert_eq!(resolved.last_migration_diagnostic(), None);
    assert!(!migration_path.exists());

    // A snapshot without presets needs no source.
    let bare = Scratch::new("preferences-tests", "tauri-source-bare");
    let bare_directory = bare.to_path_buf();
    std::fs::write(
        bare_directory.join("tauri-migration.json"),
        snapshot_at("2026-11-21T00:00:00Z", named(&["Alpha"])),
    )
    .unwrap();
    let bare_store = Store::file(bare_directory.clone()).with_source_resolver(unresolved_source);
    let bare_doc = bare_store.load().unwrap();
    assert_eq!(bare_doc.workspaces.len(), 1);
    assert!(!bare_doc.tauri_migration_presets_pending);
    assert!(!bare_directory.join("tauri-migration.json").exists());
}

#[test]
fn tauri_migration_keeps_shortcuts_for_a_workspace_gpui_already_has() {
    let root = Scratch::new("preferences-tests", "tauri-shortcuts");
    let directory = root.to_path_buf();
    let store = Store::file(directory.clone());
    let mut existing = store.load().unwrap();
    existing.workspaces.push(
        kubecove_gpui::workspaces::Draft {
            name: "Alpha",
            effective: "dev",
            group: &[],
            namespaces: &["web".into()],
            kinds: None,
        }
        .build(1, 1),
    );
    store.save(existing).unwrap();
    let snapshot = snapshot_at(
        "2026-11-21T00:00:00Z",
        json!([{
            "id": "a",
            "name": "ALPHA",
            "scope": { "clusterContext": "dev", "namespaces": ["tauri-ns"] },
            "shortcuts": [{ "id": "sc-1", "label": "Pods", "kind": "resources" }]
        }]),
    );
    std::fs::write(directory.join("tauri-migration.json"), snapshot).unwrap();

    let merged = Store::file(directory).load().unwrap();
    assert_eq!(merged.workspaces.len(), 1);
    assert_eq!(merged.workspaces[0].namespaces, vec!["web"]);
    let retained = &merged.workspace_documents[&1];
    assert_eq!(retained["spec"]["displayName"], "Alpha");
    assert_eq!(retained["spec"]["scope"]["namespaces"], json!(["web"]));
    assert_eq!(retained["spec"]["shortcuts"][0]["label"], "Pods");
}
