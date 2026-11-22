//! Failure modes across starts: a later Tauri snapshot re-merges and restores
//! entries the user deleted; GPUI modifies Tauri's file; presets are filed
//! under no kubeconfig source, keep the import open, or are lost when their
//! workspace is renamed before the source resolves; shortcuts vanish when a
//! workspace merges into a GPUI one.

use crate::scratch::Scratch;
use crate::tauri_migration::{fixed_source, scoped_preset_key, unresolved_source};
use kubecove_gpui::{preferences::Store, workspaces::transfer::API_VERSION};
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
fn tauri_migration_imports_once_and_never_modifies_the_snapshot() {
    let root = Scratch::new("preferences-tests", "tauri-once");
    let directory = root.to_path_buf();
    let migration_path = directory.join("tauri-migration.json");
    let snapshot = snapshot_at("2026-11-21T00:00:00Z", named(&["Alpha", "Beta"]));
    std::fs::write(&migration_path, &snapshot).unwrap();

    let store = Store::file(directory.clone());
    let imported = store.load().unwrap();
    assert_eq!(imported.workspaces.len(), 2);
    assert_eq!(
        imported.tauri_migration_imported.as_deref(),
        Some("2026-11-21T00:00:00Z")
    );

    // The user deletes a migrated workspace; Tauri exports a newer snapshot.
    let mut edited = imported;
    edited.workspaces.retain(|w| w.name != "Beta");
    let saved = store.save(edited).unwrap();
    let newer = snapshot_at("2026-11-22T00:00:00Z", named(&["Alpha", "Beta", "Gamma"]));
    std::fs::write(&migration_path, &newer).unwrap();

    let reloaded = Store::file(directory.clone()).load().unwrap();
    assert_eq!(reloaded.revision, saved.revision);
    assert_eq!(reloaded.workspaces.len(), 1);
    assert_eq!(reloaded.workspaces[0].name, "Alpha");
    assert_eq!(std::fs::read_to_string(&migration_path).unwrap(), newer);
    let names: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with("tauri-migration"))
        .collect();
    assert_eq!(names, vec!["tauri-migration.json"]);
}

#[test]
fn tauri_migration_keeps_presets_in_gpui_state_until_the_source_resolves() {
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
    assert!(first.tauri_migration_imported.is_some());
    assert_eq!(first.tauri_migration_pending_presets.len(), 2);
    assert!(first.saved_forward_presets.is_empty());
    assert!(
        waiting
            .last_migration_diagnostic()
            .unwrap()
            .contains("kubeconfig settings unreadable")
    );

    // Tauri's file can change or go away; the pending presets live in GPUI's
    // own document, keyed by workspace ID. The user renames one workspace,
    // deletes the other and changes a setting before the source resolves.
    std::fs::remove_file(&migration_path).unwrap();
    let mut edited = first;
    edited.preferences.show_exact_timestamps = false;
    edited.workspaces.retain(|w| w.name != "Beta");
    edited.workspaces[0].name = "Alpha renamed".into();
    let saved = waiting.save(edited).unwrap();
    assert_eq!(waiting.load().unwrap().revision, saved.revision);

    let resolved = Store::file(directory.clone()).with_source_resolver(fixed_source);
    let finished = resolved.load().unwrap();
    assert!(finished.tauri_migration_pending_presets.is_empty());
    assert!(!finished.preferences.show_exact_timestamps);
    assert_eq!(finished.saved_forward_presets.len(), 1);
    assert_eq!(
        finished.saved_forward_presets[&scoped_preset_key("dev", "Alpha renamed")].len(),
        1
    );
    assert_eq!(resolved.last_migration_diagnostic(), None);
    // Filing is done once; the next load has no import work.
    assert_eq!(
        Store::file(directory).load().unwrap().revision,
        finished.revision
    );
}

#[test]
fn tauri_migration_fills_only_empty_shortcut_slots_of_existing_workspaces() {
    let root = Scratch::new("preferences-tests", "tauri-shortcuts");
    let directory = root.to_path_buf();
    let store = Store::file(directory.clone());
    let mut existing = store.load().unwrap();
    for (id, name) in [(1, "Alpha"), (2, "Beta"), (3, "Gamma")] {
        existing.workspaces.push(
            kubecove_gpui::workspaces::Draft {
                name,
                effective: "dev",
                group: &[],
                namespaces: &["web".into()],
                kinds: None,
            }
            .build(id, id),
        );
    }
    let retained = |shortcuts: serde_json::Value| {
        json!({
            "apiVersion": API_VERSION,
            "kind": "Workspace",
            "metadata": { "name": "gpui" },
            "spec": {
                "displayName": "GPUI",
                "scope": { "clusterContext": "dev", "namespaces": ["web"], "kinds": ["Pod"], "layout": "overview" },
                "shortcuts": shortcuts,
                "portForwards": []
            }
        })
    };
    // Beta was imported through GPUI with no shortcuts; Gamma has its own.
    existing.workspace_documents.insert(2, retained(json!([])));
    existing.workspace_documents.insert(
        3,
        retained(json!([{ "id": "gpui", "label": "GPUI wins", "kind": "resources" }])),
    );
    store.save(existing).unwrap();
    let shortcut = json!([{ "id": "sc-1", "label": "Pods", "kind": "resources" }]);
    let workspaces: Vec<_> = ["ALPHA", "Beta", "Gamma"]
        .iter()
        .map(|name| {
            json!({
                "id": name,
                "name": name,
                "scope": { "clusterContext": "dev", "namespaces": ["tauri-ns"] },
                "shortcuts": shortcut
            })
        })
        .collect();
    std::fs::write(
        directory.join("tauri-migration.json"),
        snapshot_at("2026-11-21T00:00:00Z", json!(workspaces)),
    )
    .unwrap();

    let merged = Store::file(directory).load().unwrap();
    assert_eq!(merged.workspaces.len(), 3);
    assert_eq!(merged.workspaces[0].namespaces, vec!["web"]);
    let alpha = &merged.workspace_documents[&1];
    assert_eq!(alpha["spec"]["displayName"], "Alpha");
    assert_eq!(alpha["spec"]["scope"]["namespaces"], json!(["web"]));
    assert_eq!(alpha["spec"]["shortcuts"][0]["label"], "Pods");
    assert_eq!(
        merged.workspace_documents[&2]["spec"]["shortcuts"][0]["label"],
        "Pods"
    );
    let gamma = &merged.workspace_documents[&3]["spec"]["shortcuts"];
    assert_eq!(gamma.as_array().unwrap().len(), 1);
    assert_eq!(gamma[0]["label"], "GPUI wins");
}
