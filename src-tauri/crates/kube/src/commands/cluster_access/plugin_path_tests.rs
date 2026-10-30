use super::*;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

// Failure modes: GUI PATH cannot find the configured command or its child plugin;
// shell banners contaminate PATH; explicit kubeconfig PATH is overwritten;
// failed shell startup replaces a usable inherited environment.
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "kubecove path {} {}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn config() -> Config {
    let mut config = Config::new("http://127.0.0.1:1".parse().unwrap());
    config.auth_info.exec = Some(
        serde_json::from_value(serde_json::json!({
            "apiVersion":"client.authentication.k8s.io/v1", "command":"kubecove-path-fixture-cli",
            "args":["oidc-login", "get-token"], "interactiveMode":"Never"
        }))
        .unwrap(),
    );
    config
}

#[tokio::test]
async fn gui_path_finds_command_and_nested_plugin_without_changing_configuration() {
    let fixture = Fixture::new();
    fixture.script(
        "kubecove-path-fixture-cli",
        "test \"$1\" = oidc-login || exit 1\nshift\nexec kubecove-path-fixture-plugin \"$@\"",
    );
    fixture.script("kubecove-path-fixture-plugin", "test \"$1\" = get-token || exit 1\nprintf '%s' '{\"apiVersion\":\"client.authentication.k8s.io/v1\",\"kind\":\"ExecCredential\",\"status\":{\"token\":\"synthetic-path-test\",\"expirationTimestamp\":\"2099-01-01T00:00:00Z\"}}'");
    let shell = fixture.script(
        "shell",
        "printf 'startup banner\\n\\036KUBECOVE_PATH\\037%s\\n' \"$FIXTURE_BIN\"",
    );
    let mut config = config();
    recover(
        &mut config,
        Some(std::ffi::OsStr::new("/usr/bin:/bin")),
        async { read_shell_path(Command::new(shell).env("FIXTURE_BIN", &fixture.0)).await },
    )
    .await;
    assert_eq!(
        config.auth_info.exec.as_ref().unwrap().command.as_deref(),
        Some("kubecove-path-fixture-cli")
    );
    let _client = kube::client::ClientBuilder::try_from(config)
        .expect("GUI must find the command and nested plugin")
        .build();
}

#[tokio::test]
async fn explicit_path_and_non_exec_auth_never_probe_the_shell() {
    let mut config = config();
    config.auth_info.exec.as_mut().unwrap().env = Some(vec![std::collections::HashMap::from([
        ("name".into(), "PATH".into()),
        ("value".into(), "explicit-bin".into()),
    ])]);
    let before = serde_json::to_value(&config.auth_info.exec).unwrap();
    recover(&mut config, None, async {
        panic!("explicit PATH must win")
    })
    .await;
    assert_eq!(
        serde_json::to_value(&config.auth_info.exec).unwrap(),
        before
    );
    config.auth_info.exec = None;
    recover(&mut config, None, async {
        panic!("non-exec auth must not run a shell")
    })
    .await;
}

#[tokio::test]
async fn shell_failure_or_invalid_output_preserves_inherited_lookup() {
    let fixture = Fixture::new();
    for body in [
        "exit 1",
        "printf 'unrelated output\\n'",
        "printf '\\036KUBECOVE_PATH\\037\\n'",
    ] {
        let shell = fixture.script("shell", body);
        let mut config = config();
        let before = serde_json::to_value(&config.auth_info.exec).unwrap();
        recover(
            &mut config,
            Some(std::ffi::OsStr::new("/usr/bin:/bin")),
            async { read_shell_path(&mut Command::new(shell)).await },
        )
        .await;
        assert_eq!(
            serde_json::to_value(&config.auth_info.exec).unwrap(),
            before
        );
    }
}

#[tokio::test]
async fn shell_command_returns_the_exported_path() {
    let inherited = std::env::var("PATH").unwrap();
    let recovered = read_shell_path(
        Command::new("sh")
            .args(["-ic", SHELL_COMMAND])
            .env("PATH", &inherited),
    )
    .await;
    assert_eq!(recovered.as_deref(), Some(inherited.as_str()));
}

#[tokio::test]
async fn cancelled_shell_probe_releases_shared_initialization_for_retry() {
    use futures_util::FutureExt;
    let fixture = Fixture::new();
    let fifo = fixture.0.join("wait");
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let started = fixture.0.join("started");
    let shell = fixture.script(
        "shell",
        "printf x > \"$STARTED\"\nread -r value < \"$WAIT\"",
    );
    let cache = OnceCell::new();
    let mut config = config();
    {
        let recovery = recover(&mut config, None, async {
            cache
                .get_or_init(|| async {
                    read_shell_path(
                        Command::new(shell)
                            .env("STARTED", &started)
                            .env("WAIT", &fifo),
                    )
                    .await
                })
                .await
                .clone()
        });
        tokio::pin!(recovery);
        tokio::select! {
            () = &mut recovery => panic!("shell must still be waiting"),
            () = async { while !started.exists() { tokio::task::yield_now().await; } } => {}
        }
    }
    assert!(cache.get().is_none());
    let retry = cache
        .get_or_init(|| async { Some("/fixture/bin".to_owned()) })
        .now_or_never()
        .expect("cancelled initialization must not hold the cache");
    assert_eq!(retry.as_deref(), Some("/fixture/bin"));
    assert!(config.auth_info.exec.unwrap().env.is_none());
}
