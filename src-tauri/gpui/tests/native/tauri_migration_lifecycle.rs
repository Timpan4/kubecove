//! Failure modes across starts: a later Tauri snapshot re-merges and restores
//! entries the user deleted; GPUI modifies Tauri's file; presets with no
//! kubeconfig source keep the import open or vanish without a report;
//! shortcuts vanish when a workspace merges into a GPUI one, or a retained
//! document that is not a single `Workspace` crashes the merge.

use crate::scratch::Scratch;
use crate::tauri_migration::{fixed_source, unresolved_source};
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
fn tauri_migration_skips_and_reports_presets_without_a_kubeconfig_source() {
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

    let importer = Store::file(directory.clone()).with_source_resolver(unresolved_source);
    let imported = importer.load().unwrap();
    assert_eq!(imported.workspaces.len(), 2);
    assert!(imported.preferences.show_exact_timestamps);
    assert!(imported.tauri_migration_imported.is_some());
    assert!(imported.saved_forward_presets.is_empty());
    // The resolver's error text ("kubeconfig settings unreadable") can quote
    // kubeconfig contents, so neither stderr nor the stored notice repeats it.
    let report = "Imported tauri-migration.json with 2 saved port forward(s) skipped because the active kubeconfig source could not be resolved.";
    assert_eq!(
        importer.last_migration_diagnostic().as_deref(),
        Some(report)
    );
    // The report is stored with the import, so Settings shows it after restart.
    assert_eq!(imported.tauri_migration_notice.as_deref(), Some(report));

    // The import finished in that one write. A resolvable source later does
    // not reopen it, and nothing is written.
    let resolved = Store::file(directory).with_source_resolver(fixed_source);
    let reloaded = resolved.load().unwrap();
    assert_eq!(reloaded.revision, imported.revision);
    assert!(reloaded.saved_forward_presets.is_empty());
    assert_eq!(reloaded.tauri_migration_notice.as_deref(), Some(report));
    assert_eq!(resolved.last_migration_diagnostic(), None);
    assert_eq!(std::fs::read_to_string(&migration_path).unwrap(), snapshot);
}

/// Failure mode: the report of what the import skipped never reaches the user,
/// vanishes when dismissing it fails to save, or comes back after a dismissal.
#[gpui_kit::test]
fn settings_shows_the_tauri_import_notice_until_dismissed(cx: &mut gpui_kit::TestAppContext) {
    use crate::*;
    let backend = std::sync::Arc::new(crate::Controlled::default());
    let report = "Imported tauri-migration.json with 1 saved port forward(s) skipped.";
    backend.argo.document.lock().unwrap().tauri_migration_notice = Some(report.into());
    let handle = crate::open(cx, backend.clone());
    let click = |cx: &mut gpui_kit::TestAppContext, id: &'static str| {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click(id, cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    click(cx, "open-settings");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("settings-notice:tauri-import").is_some());
    })
    .unwrap();
    let next_save = backend.argo.saves.load(Ordering::SeqCst) + 1;
    backend.argo.fail_save.store(next_save, Ordering::SeqCst);
    click(cx, "settings-button:Dismiss Tauri import notice");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("settings-notice:tauri-import").is_some());
    })
    .unwrap();
    assert_eq!(
        backend
            .argo
            .document
            .lock()
            .unwrap()
            .tauri_migration_notice
            .as_deref(),
        Some(report)
    );
    click(cx, "settings-button:Dismiss Tauri import notice");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("settings-notice:tauri-import").is_none());
    })
    .unwrap();
    assert_eq!(
        backend.argo.document.lock().unwrap().tauri_migration_notice,
        None
    );
}

#[test]
fn tauri_migration_fills_only_empty_shortcut_slots_of_existing_workspaces() {
    let root = Scratch::new("preferences-tests", "tauri-shortcuts");
    let directory = root.to_path_buf();
    let store = Store::file(directory.clone());
    let mut existing = store.load().unwrap();
    for (id, name) in [(1, "Alpha"), (2, "Beta"), (3, "Gamma"), (4, "Delta")] {
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
    // A valid retained list document has no top-level `spec` to fill.
    let list = json!({
        "apiVersion": API_VERSION,
        "kind": "WorkspaceList",
        "metadata": { "name": "kubecove-workspaces" },
        "items": [retained(json!([]))]
    });
    existing.workspace_documents.insert(4, list.clone());
    store.save(existing).unwrap();
    let shortcut = json!([{ "id": "sc-1", "label": "Pods", "kind": "resources" }]);
    let workspaces: Vec<_> = ["ALPHA", "Beta", "Gamma", "Delta"]
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
    assert!(merged.tauri_migration_imported.is_some());
    assert_eq!(merged.workspaces.len(), 4);
    assert_eq!(merged.workspace_documents[&4], list);
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
