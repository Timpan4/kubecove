use super::*;

#[test]
fn drops_credentials_and_invalid_targets_after_projection() {
    let timestamp = "2026-11-21T00:00:00Z";
    let review = json!({"clusterContext":"dev","objectKey":"Roles:Role:token:reader",
        "evidenceFingerprint":"rbac-v1-abc","disposition":"expected","reviewedAt":timestamp});
    for note in [
        "client-key-data",
        "client-certificate-data",
        "certificate-authority-data",
        "Authorization",
        "-----BEGIN RSA PRIVATE KEY-----",
        "ghp_canary",
    ] {
        let mut item = review.clone();
        item["note"] = format!("Review copy: {note}: canary").into();
        let result = sanitize::workspaces(&json!({"workspaces":[{"id":"w","name":"w","scope":{},
            "rbacReviews":[item]}]}));
        assert_eq!(result["workspaces"][0]["rbacReviews"], json!([]), "{note}");
    }
    let preset = json!({"id":"api","clusterContext":"dev","namespace":"apps","serviceName":"api",
        "servicePort":443,"createdAt":timestamp,"updatedAt":timestamp});
    for (field, bad) in [
        ("id", json!("")),
        ("serviceName", Value::Null),
        ("namespace", json!(" ")),
        ("clusterContext", json!("token: canary")),
        ("servicePort", json!(65536)),
        ("servicePort", json!(0)),
        ("localPort", json!(-1)),
    ] {
        let mut item = preset.clone();
        item[field] = bad;
        let result = sanitize::workspaces(&json!({"workspaces":[{"id":"w","name":"w","scope":{},
            "portForwards":[item]}]}));
        assert_eq!(
            result["workspaces"][0]["portForwards"],
            json!([]),
            "{field}"
        );
    }
}

#[test]
fn handoff_keeps_scope_and_review_text_but_excludes_credentials_at_each_depth() {
    let directory = tempfile::tempdir().unwrap();
    let timestamp = "2026-11-21T00:00:00Z";
    let review = json!({"clusterContext":"dev","objectKey":"Roles:Role:token:reader",
        "evidenceFingerprint":"rbac-v1-abc","disposition":"expected","reviewedAt":timestamp,
        "note":"Approved for on-call"});
    let preset = json!({"id":"metrics","clusterContext":"dev","namespace":"apps",
        "serviceName":"metrics","servicePort":9090,"createdAt":timestamp,"updatedAt":timestamp});
    let input = || {
        MigrationInput {
        settings: Some(json!({"state":{
            "showUsageFooter":true, "token":"canary",
            "argoProfiles":[
                {"id":"local","transport":"connected","rememberCredential":true,"endpoint":{"kind":"externalHttps","url":"https://argo.example","auth":{"password":"canary"}},"cookies":{"session":"canary"}},
                {"id":"bad","url":"https://admin:canary@argo.example"},
                {"id":"externalHttps:https://argo.example?access_token=canary","url":"https://argo.example"}],
            "argoConnectionPreferences":{"dev":{"kind":"connected","profileId":"local","headers":{"Authorization":"canary"}}}
        }}).to_string()),
        workspaces: Some(json!({"state":{"workspaces":[{"id":"dev","name":"Development","scope":{"clusterContext":"dev","namespaces":["apps"],"kinds":[{"kind":"Pod","privateKey":"canary"}]},
            "portForwards":[preset,{"id":"broken","lastError":"canary"}],
            "entryPoints":{"pinned":[{"name":"api","payload":"canary"}],"recent":[{"name":"web"}]},
            "rbacReviews":[review,{"note":"orphan"}]}]}}).to_string()),
    }
    };
    for file in ["kubeconfig-sources.json", "mcp-settings.json", "mcp-token"] {
        fs::write(directory.path().join(file), b"original").unwrap();
    }
    assert!(write(directory.path(), input()).unwrap());
    let path = directory.path().join("tauri-migration.json");
    let bytes = fs::read_to_string(&path).unwrap();
    assert!(!bytes.contains("canary"));
    let entries = &serde_json::from_str::<Value>(&bytes).unwrap()["entries"];
    let settings = &entries["kubecove-settings"]["state"];
    assert_eq!(settings["showUsageFooter"], true);
    assert_eq!(settings["argoProfiles"].as_array().unwrap().len(), 1);
    assert_eq!(
        settings["argoConnectionPreferences"]["dev"]["profileId"],
        "local"
    );
    let workspace = &entries["kubecove-workspaces"]["state"]["workspaces"][0];
    assert_eq!(workspace["portForwards"], json!([preset]));
    assert_eq!(workspace["rbacReviews"], json!([review]));
    let profile: crate::models::ArgoConnectionProfile =
        serde_json::from_value(settings["argoProfiles"][0].clone()).unwrap();
    assert_eq!(profile.transport, "connected");
    assert!(profile.remember_credential);
    assert_eq!(workspace["entryPoints"]["pinned"][0]["name"], "api");
    assert_eq!(workspace["entryPoints"]["recent"][0]["name"], "web");
    assert!(!write(directory.path(), input()).unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
    let mut invalid = input();
    invalid.settings = Some("{".into());
    assert!(write(directory.path(), invalid).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
    let mut changed = input();
    changed.workspaces = None;
    assert!(write(directory.path(), changed).unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    for file in ["kubeconfig-sources.json", "mcp-settings.json", "mcp-token"] {
        assert_eq!(fs::read(directory.path().join(file)).unwrap(), b"original");
    }
}
