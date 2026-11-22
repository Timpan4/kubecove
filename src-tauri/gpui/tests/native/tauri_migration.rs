//! Failure modes: fresh Tauri 0.15.1 migration loses preferences/workspaces/reviews/pins/Argo;
//! existing GPUI entries get clobbered on merge; migration re-runs after rename; corrupt
//! migration file fails startup or gets deleted; secrets enter live-sessions.json.

use crate::scratch::Scratch;
use kubecove_gpui::preferences::{ConnectionPreference, Endpoint, LogLayout, Store};
use kubecove_lib::models::{AppError, AppErrorKind};
use serde_json::json;

pub(crate) const SOURCE: &str = "kubeconfigEnv=KUBECONFIG";

pub(crate) fn fixed_source() -> Result<String, AppError> {
    Ok(SOURCE.into())
}

pub(crate) fn unresolved_source() -> Result<String, AppError> {
    Err(AppError::new(
        "kubeconfig settings unreadable",
        AppErrorKind::Io,
    ))
}

/// The key `Inspector::preset_key` builds once hydration reports `SOURCE`.
pub(crate) fn scoped_preset_key(context: &str, workspace: &str) -> String {
    serde_json::to_string(&(Some(SOURCE), context, workspace)).unwrap()
}

#[test]
fn tauri_migration_fresh_import_and_idempotency() {
    let root = Scratch::new("preferences-tests", "tauri-fresh");
    let directory = root.to_path_buf();
    let timestamp = "2026-11-21T00:00:00Z";
    let snapshot = json!({
        "schemaVersion": 1,
        "sourceVersion": "0.15.1",
        "exportedAt": timestamp,
        "entries": {
            "kubecove-settings": {
                "version": 0,
                "state": {
                    "showExactTimestamps": true,
                    "showUsageFooter": true,
                    "redactSecrets": false,
                    "gitOpsViewMode": "list",
                    "helmViewMode": "list",
                    "timestampTimezone": "utc",
                    "yamlViewModeDefault": "applyClean",
                    "yamlEncodingDefault": "kyaml",
                    "yamlDiffStyle": "git",
                    "logLayout": "oneLine",
                    "resourceKindLanes": { "first": ["Secret"], "normal": ["*"], "last": ["Pod"] },
                    "argoProfiles": [
                        {
                            "id": "argo-ext",
                            "transport": "connected",
                            "workspaceId": "tauri-ws-1",
                            "clusterContext": "prod",
                            "kubeconfigSourceKey": "default",
                            "rememberCredential": true,
                            "endpoint": { "kind": "externalHttps", "url": "https://argo.example.test" }
                        },
                        {
                            "id": "argo-tun",
                            "transport": "connected",
                            "clusterContext": "staging",
                            "rememberCredential": false,
                            "endpoint": {
                                "kind": "serviceTunnel",
                                "namespace": "argocd",
                                "serviceName": "argocd-server",
                                "servicePort": 443,
                                "scheme": "https",
                                "rootPath": "/argo"
                            }
                        }
                    ],
                    "argoConnectionPreferences": {
                        "tauri-ws-1": { "kind": "connected", "profileId": "argo-ext" },
                        "staging": { "kind": "kubernetes" }
                    }
                }
            },
            "kubecove-workspaces": {
                "version": 0,
                "state": {
                    "workspaces": [
                        {
                            "id": "tauri-ws-1",
                            "name": "Production",
                            "sharedKey": "production",
                            "scope": {
                                "clusterContext": "prod",
                                "layout": "resources",
                                "namespaces": ["payments", "checkout"],
                                "kinds": [
                                    "Pod",
                                    {
                                        "group": "argoproj.io",
                                        "version": "v1alpha1",
                                        "apiVersion": "argoproj.io/v1alpha1",
                                        "kind": "Application",
                                        "plural": "applications",
                                        "namespaced": true
                                    },
                                    {
                                        "group": "",
                                        "version": "v1",
                                        "apiVersion": "v1",
                                        "kind": "ResourceQuota",
                                        "plural": "resourcequotas",
                                        "namespaced": true
                                    }
                                ],
                                "clusterGroup": {
                                    "id": "grp-1",
                                    "name": "Prod Fleet",
                                    "members": ["prod", "prod-eu"]
                                }
                            },
                            "shortcuts": [
                                { "id": "sc-1", "label": "Payments", "kind": "namespace", "namespace": "payments" }
                            ],
                            "portForwards": [
                                {
                                    "id": "pf-1",
                                    "clusterContext": "prod",
                                    "namespace": "payments",
                                    "serviceName": "api",
                                    "servicePort": 8080,
                                    "localPort": 18080,
                                    "label": "Payments API",
                                    "createdAt": timestamp,
                                    "updatedAt": timestamp
                                }
                            ],
                            "rbacReviews": [
                                {
                                    "clusterContext": "prod",
                                    "objectKey": "Roles:Role:token:reader",
                                    "evidenceFingerprint": "rbac-v1-abc123",
                                    "disposition": "expected",
                                    "note": "Approved for on-call",
                                    "reviewedAt": timestamp
                                }
                            ],
                            "entryPoints": {
                                "pinned": [
                                    {
                                        "kind": "resource",
                                        "clusterContext": "prod",
                                        "namespace": "payments",
                                        "name": "api",
                                        "resourceKind": "Deployment",
                                        "apiVersion": "apps/v1",
                                        "lastVisitedAt": timestamp
                                    }
                                ],
                                "recent": [
                                    {
                                        "kind": "namespace",
                                        "clusterContext": "prod",
                                        "name": "payments",
                                        "lastVisitedAt": timestamp
                                    }
                                ]
                            }
                        }
                    ]
                }
            }
        }
    });
    let migration_path = directory.join("tauri-migration.json");
    std::fs::write(&migration_path, snapshot.to_string()).unwrap();

    let store = Store::file(directory.clone()).with_source_resolver(fixed_source);
    let loaded = store.load().unwrap();
    assert_eq!(loaded.tauri_migration_imported.as_deref(), Some(timestamp));
    // The "staging" connection key names no migrated workspace, so it is skipped.
    assert_eq!(
        store.last_migration_diagnostic().as_deref(),
        Some("Imported tauri-migration.json with 1 invalid or unsafe field(s) skipped.")
    );
    // Tauri's file is read-only input.
    assert_eq!(
        std::fs::read_to_string(&migration_path).unwrap(),
        snapshot.to_string()
    );
    assert!(directory.join("live-sessions.json").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(directory.join("live-sessions.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    assert!(loaded.preferences.show_exact_timestamps);
    assert!(loaded.preferences.show_usage_footer);
    assert!(!loaded.preferences.redact_secrets);
    assert_eq!(loaded.preferences.log_layout, LogLayout::OneLine);
    assert_eq!(loaded.preferences.resource_kind_lanes.first, vec!["Secret"]);
    assert_eq!(loaded.preferences.argo_profiles.len(), 2);
    assert_eq!(
        loaded.preferences.argo_profiles[0].workspace_id.as_deref(),
        Some("1")
    );
    assert_eq!(
        loaded.preferences.argo_profiles[1].endpoint,
        Endpoint::ServiceTunnel {
            namespace: "argocd".into(),
            service_name: "argocd-server".into(),
            service_port: 443,
            scheme: "https".into(),
            root_path: Some("/argo".into()),
            tls_server_name: None,
        }
    );
    // GPUI keys connection preferences by its own workspace ID; "staging" names no workspace.
    assert_eq!(
        loaded.preferences.argo_connection_preferences,
        [(
            "1".to_owned(),
            ConnectionPreference::Connected {
                profile_id: "argo-ext".into()
            }
        )]
        .into()
    );

    assert_eq!(loaded.workspaces.len(), 1);
    let ws = &loaded.workspaces[0];
    assert_eq!(ws.id, 1);
    assert_eq!(ws.name, "Production");
    assert_eq!(ws.context, "prod");
    assert_eq!(ws.group, vec!["prod-eu"]);
    assert_eq!(ws.group_name.as_deref(), Some("Prod Fleet"));
    assert_eq!(ws.namespaces, vec!["checkout", "payments"]);
    // A discovered core-group kind carries the empty group and is kept.
    assert_eq!(ws.kinds.len(), 3);
    assert_eq!(ws.reviews.len(), 1);
    assert_eq!(ws.reviews[0].note, "Approved for on-call");
    assert!(loaded.workspace_documents.contains_key(&1));
    assert_eq!(loaded.workspace_entry_points[&1].pinned.len(), 1);
    assert_eq!(loaded.workspace_entry_points[&1].recent.len(), 1);
    let preset_key = scoped_preset_key("prod", "Production");
    assert_eq!(loaded.saved_forward_presets.len(), 1);
    assert_eq!(loaded.saved_forward_presets[&preset_key].len(), 1);
    assert_eq!(
        loaded.saved_forward_presets[&preset_key][0].label,
        "Payments API"
    );

    // Idempotency: editing GPUI preferences and reloading never re-imports the snapshot.
    let mut edited = loaded;
    edited.preferences.show_exact_timestamps = false;
    let saved = store.save(edited).unwrap();
    let reloaded = Store::file(directory).load().unwrap();
    assert!(!reloaded.preferences.show_exact_timestamps);
    assert_eq!(reloaded.workspaces[0].kinds.len(), 3);
    assert_eq!(reloaded.revision, saved.revision);
}

#[test]
fn tauri_migration_merges_without_clobbering_existing_gpui_data() {
    let root = Scratch::new("preferences-tests", "tauri-merge");
    let directory = root.to_path_buf();
    let timestamp = "2026-11-21T00:00:00Z";
    let store = Store::file(directory.clone());

    let mut existing = store.load().unwrap();
    existing.preferences.show_exact_timestamps = true;
    existing.preferences.show_usage_footer = false;
    existing
        .preferences
        .argo_profiles
        .push(kubecove_gpui::preferences::ArgoProfile {
            id: "shared-profile".into(),
            endpoint: Endpoint::ExternalHttps {
                url: "https://gpui-wins.example.test".into(),
            },
            cluster_context: Some("prod".into()),
            workspace_id: Some("1".into()),
            kubeconfig_source_key: None,
            remember_credential: true,
        });
    existing
        .preferences
        .argo_connection_preferences
        .insert("1".into(), ConnectionPreference::Kubernetes);
    let mut gpui_ws = kubecove_gpui::workspaces::Draft {
        name: "Production",
        effective: "prod",
        group: &[],
        namespaces: &["gpui-ns".into()],
        kinds: None,
    }
    .build(1, 1);
    gpui_ws.reviews = serde_json::from_value(json!([{
        "cluster_context": "prod",
        "object_key": "Roles:Role:gpui-ns:admin",
        "evidence_fingerprint": "rbac-v1-gpui",
        "disposition": "anomalous",
        "note": "GPUI note wins",
        "reviewed_at": timestamp
    }]))
    .unwrap();
    existing.workspaces.push(gpui_ws);
    store.save(existing).unwrap();

    let snapshot = json!({
        "schemaVersion": 1,
        "sourceVersion": "0.15.1",
        "exportedAt": timestamp,
        "entries": {
            "kubecove-settings": {
                "version": 0,
                "state": {
                    "showExactTimestamps": false,
                    "showUsageFooter": true,
                    "argoProfiles": [
                        {
                            "id": "shared-profile",
                            "transport": "connected",
                            "endpoint": { "kind": "externalHttps", "url": "https://tauri-loses.example.test" }
                        },
                        {
                            "id": "shared-profile",
                            "transport": "connected",
                            "endpoint": { "kind": "externalHttps", "url": "https://tauri-loses-again.example.test" }
                        },
                        {
                            "id": "orphan-profile",
                            "transport": "connected",
                            "workspaceId": "tauri-unknown",
                            "endpoint": { "kind": "externalHttps", "url": "https://tauri-orphan.example.test" }
                        },
                        {
                            "id": "new-profile",
                            "transport": "connected",
                            "workspaceId": "tauri-staging",
                            "endpoint": { "kind": "externalHttps", "url": "https://tauri-new.example.test" }
                        },
                        {
                            "id": "dropped-profile",
                            "transport": "connected",
                            "endpoint": { "kind": "externalHttps", "url": "https://admin:pw@tauri-dropped.example.test" }
                        }
                    ],
                    "argoConnectionPreferences": {
                        "tauri-prod": { "kind": "connected", "profileId": "shared-profile" },
                        "tauri-staging": { "kind": "connected", "profileId": "new-profile" },
                        "tauri-dev": { "kind": "connected", "profileId": "dropped-profile" },
                        "tauri-qa": { "kind": "connected", "profileId": "shared-profile" },
                        "tauri-unknown": { "kind": "kubernetes" }
                    }
                }
            },
            "kubecove-workspaces": {
                "version": 0,
                "state": {
                    "workspaces": [
                        {
                            "id": "tauri-prod",
                            "name": "PRODUCTION",
                            "scope": { "clusterContext": "prod", "namespaces": ["tauri-ns"] },
                            "portForwards": [
                                {
                                    "clusterContext": "prod",
                                    "namespace": "payments",
                                    "serviceName": "api",
                                    "servicePort": 8080
                                }
                            ],
                            "rbacReviews": [
                                {
                                    "clusterContext": "prod",
                                    "objectKey": "Roles:Role:gpui-ns:admin",
                                    "evidenceFingerprint": "rbac-v1-tauri",
                                    "disposition": "expected",
                                    "note": "Tauri note loses",
                                    "reviewedAt": timestamp
                                },
                                {
                                    "clusterContext": "prod",
                                    "objectKey": "Roles:Role:gpui-ns:viewer",
                                    "evidenceFingerprint": "rbac-v1-new",
                                    "disposition": "expected",
                                    "note": "Merged new review",
                                    "reviewedAt": timestamp
                                }
                            ]
                        },
                        {
                            "id": "tauri-staging",
                            "name": "Staging",
                            "scope": { "clusterContext": "staging", "namespaces": ["web"] }
                        },
                        {
                            "id": "tauri-dev",
                            "name": "Dev",
                            "scope": { "clusterContext": "dev" }
                        },
                        {
                            "id": "tauri-qa",
                            "name": "QA",
                            "scope": { "clusterContext": "qa" }
                        }
                    ]
                }
            }
        }
    });
    std::fs::write(directory.join("tauri-migration.json"), snapshot.to_string()).unwrap();

    let importer = Store::file(directory).with_source_resolver(fixed_source);
    let merged = importer.load().unwrap();
    // Each losing entry counts: two "shared-profile" definitions and the two
    // selections of it (Production and QA).
    assert!(
        importer
            .last_migration_diagnostic()
            .unwrap()
            .contains("4 Argo CD entr(ies) kept the existing GPUI version")
    );
    assert!(merged.preferences.show_exact_timestamps);
    assert!(!merged.preferences.show_usage_footer);
    assert_eq!(merged.preferences.argo_profiles.len(), 2);
    assert_eq!(
        merged.preferences.argo_profiles[0].endpoint,
        Endpoint::ExternalHttps {
            url: "https://gpui-wins.example.test".into()
        }
    );
    // A profile bound to an unmapped Tauri workspace could never be selected,
    // so only "new-profile" is imported, bound to Staging's GPUI ID.
    assert_eq!(merged.preferences.argo_profiles[1].id, "new-profile");
    assert_eq!(
        merged.preferences.argo_profiles[1].workspace_id.as_deref(),
        Some("2")
    );
    // Staging maps to GPUI workspace 2. Dev's profile was rejected, so its
    // selection is dropped. QA selected Tauri's "shared-profile", whose ID GPUI
    // already holds with another endpoint, so that selection is dropped too.
    // The unknown Tauri workspace has no GPUI key.
    assert_eq!(
        merged.preferences.argo_connection_preferences,
        [
            ("1".to_owned(), ConnectionPreference::Kubernetes),
            (
                "2".to_owned(),
                ConnectionPreference::Connected {
                    profile_id: "new-profile".into()
                }
            ),
        ]
        .into()
    );
    assert_eq!(merged.workspaces.len(), 4);
    assert_eq!(merged.workspaces[0].name, "Production");
    assert_eq!(merged.workspaces[0].namespaces, vec!["gpui-ns"]);
    assert_eq!(merged.workspaces[0].reviews.len(), 2);
    assert_eq!(merged.workspaces[0].reviews[0].note, "GPUI note wins");
    assert_eq!(merged.workspaces[0].reviews[1].note, "Merged new review");
    assert_eq!(merged.workspaces[1].name, "Staging");
    assert_eq!(merged.workspaces[1].id, 2);
    // A case-insensitive match files presets under the existing workspace's name.
    let preset_key = scoped_preset_key("prod", "Production");
    assert_eq!(merged.saved_forward_presets.len(), 1);
    assert_eq!(merged.saved_forward_presets[&preset_key].len(), 1);
}

#[test]
fn tauri_migration_imports_only_preference_keys_gpui_has_not_stored() {
    let root = Scratch::new("preferences-tests", "tauri-sparse");
    let directory = root.to_path_buf();
    // GPUI stored showUsageFooter explicitly at its default and never stored
    // showExactTimestamps.
    let mut stored = serde_json::to_value(kubecove_gpui::preferences::Document::default()).unwrap();
    stored["preferences"] = json!({ "showUsageFooter": false });
    std::fs::write(directory.join("live-sessions.json"), stored.to_string()).unwrap();
    let snapshot = json!({
        "schemaVersion": 1,
        "exportedAt": "2026-11-21T00:00:00Z",
        "entries": {
            "kubecove-settings": {
                "version": 0,
                "state": { "showUsageFooter": true, "showExactTimestamps": true }
            }
        }
    });
    std::fs::write(directory.join("tauri-migration.json"), snapshot.to_string()).unwrap();

    let merged = Store::file(directory).load().unwrap();
    assert!(!merged.preferences.show_usage_footer);
    assert!(merged.preferences.show_exact_timestamps);
}

#[test]
fn tauri_migration_corrupt_and_partial_handling() {
    let root = Scratch::new("preferences-tests", "tauri-corrupt");
    let directory = root.to_path_buf();
    let migration_path = directory.join("tauri-migration.json");

    for raw in [
        "{not-json",
        r#"{"schemaVersion":2,"exportedAt":"2026-11-21T00:00:00Z","entries":{}}"#,
        r#"{"schemaVersion":1,"exportedAt":"2026-11-21T00:00:00Z","entries":"bad"}"#,
        r#"{"schemaVersion":1,"entries":{}}"#,
        r#"{"schemaVersion":1,"exportedAt":"yesterday","entries":{}}"#,
    ] {
        std::fs::write(&migration_path, raw).unwrap();
        let store = Store::file(directory.clone());
        let doc = store.load().unwrap();
        assert_eq!(doc.version, 2);
        assert!(store.last_migration_diagnostic().is_some());
        assert_eq!(doc.tauri_migration_imported, None);
        assert_eq!(std::fs::read_to_string(&migration_path).unwrap(), raw);
    }

    let partial = json!({
        "schemaVersion": 1,
        "exportedAt": "2026-11-21T00:00:00Z",
        "entries": {
            "kubecove-settings": {
                "version": 0,
                "state": {
                    "showUsageFooter": true,
                    "logLayout": "invalid-layout",
                    "resourceKindLanes": { "first": ["Pod"], "normal": ["Pod"], "last": [] }
                }
            },
            "kubecove-workspaces": {
                "version": 0,
                "state": {
                    "workspaces": [
                        { "id": "bad", "name": "Missing scope" },
                        { "id": "ok", "name": "Valid", "scope": { "clusterContext": "dev" } }
                    ]
                }
            }
        }
    });
    std::fs::write(&migration_path, partial.to_string()).unwrap();
    let store = Store::file(directory.clone());
    let doc = store.load().unwrap();
    assert!(doc.preferences.show_usage_footer);
    assert_eq!(doc.preferences.log_layout, LogLayout::Stacked);
    assert_eq!(doc.workspaces.len(), 1);
    assert_eq!(doc.workspaces[0].name, "Valid");
    assert!(store.last_migration_diagnostic().is_some());
    assert!(doc.tauri_migration_imported.is_some());
}

#[test]
fn tauri_migration_never_persists_secrets() {
    let root = Scratch::new("preferences-tests", "tauri-secrets");
    let directory = root.to_path_buf();
    let timestamp = "2026-11-21T00:00:00Z";
    let snapshot = json!({
        "schemaVersion": 1,
        "sourceVersion": "0.15.1",
        "exportedAt": timestamp,
        "entries": {
            "kubecove-settings": {
                "version": 0,
                "state": {
                    "showUsageFooter": true,
                    "token": "canary-root-token",
                    "password": "canary-root-password",
                    "argoProfiles": [
                        {
                            "id": "good-argo",
                            "transport": "connected",
                            "workspaceId": "ws-1",
                            "rememberCredential": true,
                            "token": "canary-profile-token",
                            "endpoint": {
                                "kind": "externalHttps",
                                "url": "https://argo.example.test",
                                "auth": { "password": "canary-nested-password" }
                            }
                        },
                        {
                            "id": "bad-userinfo",
                            "transport": "connected",
                            "endpoint": { "kind": "externalHttps", "url": "https://admin:canary-url-pw@argo.example.test" }
                        },
                        {
                            "id": "bad-query",
                            "transport": "connected",
                            "endpoint": { "kind": "externalHttps", "url": "https://argo.example.test?access_token=canary-query" }
                        }
                    ],
                    "argoConnectionPreferences": {
                        "token": { "kind": "automatic" },
                        "ws-1": { "kind": "connected", "profileId": "good-argo", "headers": { "Authorization": "Bearer canary-hdr" } }
                    }
                }
            },
            "kubecove-workspaces": {
                "version": 0,
                "state": {
                    "workspaces": [
                        {
                            "id": "ws-1",
                            "name": "Clean Workspace",
                            "scope": { "clusterContext": "dev", "namespaces": ["apps"] },
                            "rbacReviews": [
                                {
                                    "clusterContext": "dev",
                                    "objectKey": "Roles:Role:apps:reader",
                                    "evidenceFingerprint": "rbac-v1-bad1",
                                    "disposition": "expected",
                                    "note": "Authorization: Bearer canary-note-bearer",
                                    "reviewedAt": timestamp
                                },
                                {
                                    "clusterContext": "dev",
                                    "objectKey": "Roles:Role:apps:writer",
                                    "evidenceFingerprint": "rbac-v1-bad2",
                                    "disposition": "expected",
                                    "note": "ghp_canarygithubtoken123",
                                    "reviewedAt": timestamp
                                },
                                {
                                    "clusterContext": "dev",
                                    "objectKey": "Roles:Role:apps:safe",
                                    "evidenceFingerprint": "rbac-v1-good",
                                    "disposition": "expected",
                                    "note": "Safe operational note",
                                    "reviewedAt": timestamp
                                }
                            ]
                        }
                    ]
                }
            }
        }
    });
    std::fs::write(directory.join("tauri-migration.json"), snapshot.to_string()).unwrap();

    let store = Store::file(directory.clone());
    let doc = store.load().unwrap();
    let persisted = std::fs::read_to_string(directory.join("live-sessions.json")).unwrap();
    assert!(!persisted.contains("canary"));
    assert_eq!(doc.preferences.argo_profiles.len(), 1);
    assert_eq!(doc.preferences.argo_profiles[0].id, "good-argo");
    assert!(
        !doc.preferences
            .argo_connection_preferences
            .contains_key("token")
    );
    assert!(
        doc.preferences
            .argo_connection_preferences
            .contains_key("1")
    );
    assert_eq!(doc.workspaces.len(), 1);
    assert_eq!(doc.workspaces[0].reviews.len(), 1);
    assert_eq!(doc.workspaces[0].reviews[0].note, "Safe operational note");
    Store::file(directory).load().unwrap();
}
