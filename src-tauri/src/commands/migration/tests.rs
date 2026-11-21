use super::*;

#[test]
fn handoff_keeps_scope_and_review_text_but_excludes_credentials_at_each_depth() {
    let directory = tempfile::tempdir().unwrap();
    let input = || {
        MigrationInput {
        settings: Some(json!({"state":{
            "showUsageFooter":true, "token":"canary",
            "argoProfiles":[
                {"id":"local","endpoint":{"kind":"externalHttps","url":"https://argo.example","auth":{"password":"canary"}},"cookies":{"session":"canary"}},
                {"id":"bad","url":"https://admin:canary@argo.example"},
                {"id":"externalHttps:https://argo.example?access_token=canary","url":"https://argo.example"}],
            "argoConnectionPreferences":{"dev":{"kind":"connected","profileId":"local","headers":{"Authorization":"canary"}}}
        }}).to_string()),
        workspaces: Some(json!({"state":{"workspaces":[{"id":"dev","name":"Development","scope":{"clusterContext":"dev","namespaces":["apps"],"kinds":[{"kind":"Pod","privateKey":"canary"}]},
            "portForwards":[{"id":"metrics","lastError":"canary"}],
            "entryPoints":{"pinned":[{"name":"api","payload":"canary"}],"recent":[{"name":"web"}]},
            "rbacReviews":[{"note":"Approved for on-call","cookie":"canary"},{"note":"Authorization: Bearer canary"},
                           {"note":"-----BEGIN PRIVATE KEY-----canary"},{"note":"ghp_canary"}]}]}}).to_string()),
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
    assert_eq!(
        workspace["rbacReviews"],
        json!([{"note":"Approved for on-call"}])
    );
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
