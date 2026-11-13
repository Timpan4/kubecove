#![cfg(any(target_os = "linux", target_os = "macos"))]

use kubecove_gpui::backend::{Backend, NativeBackend};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// Failure modes: the GPUI build omits shell PATH recovery; only the outer
// command is found; a credential runs but never authenticates a Kubernetes read.
#[test]
fn kubeconfig_plugin_authenticates_with_a_desktop_path() {
    if let Some(directory) = std::env::var_os("KUBECOVE_PLUGIN_FIXTURE") {
        run_backend(PathBuf::from(directory));
        return;
    }
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("credential-plugin-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let fixture = Fixture(directory);
    let bin = fixture.0.join("bin");
    fs::create_dir(&bin).unwrap();
    script(
        &bin.join("kubecove-fixture-kubectl"),
        "test \"$1\" = oidc-login || exit 1\nshift\nexec kubecove-fixture-oidc \"$@\"",
    );
    script(
        &bin.join("kubecove-fixture-oidc"),
        r#"test "$1" = get-token || exit 1
printf '%s' '{"apiVersion":"client.authentication.k8s.io/v1","kind":"ExecCredential","status":{"token":"synthetic-oidc-fixture","expirationTimestamp":"2099-01-01T00:00:00Z"}}'"#,
    );
    let shell = fixture.0.join("shell");
    script(
        &shell,
        "printf '\\036KUBECOVE_PATH\\037%s\\n' \"$KUBECOVE_PLUGIN_FIXTURE/bin\"",
    );
    // A child isolates the process environment and shell-path cache from other tests.
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "credential_plugin::kubeconfig_plugin_authenticates_with_a_desktop_path",
            "--nocapture",
        ])
        .env("KUBECOVE_PLUGIN_FIXTURE", &fixture.0)
        .env("SHELL", shell)
        .env("PATH", "/usr/bin:/bin")
        .env("KUBECONFIG", fixture.0.join("config.json"))
        .output()
        .unwrap();
    let evidence =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../e2e/artifacts/credential-plugin");
    fs::create_dir_all(&evidence).unwrap();
    let report = format!(
        "status: {}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(evidence.join("result.txt"), &report).unwrap();
    assert!(output.status.success(), "{report}");
}

fn run_backend(directory: PathBuf) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = serde_json::Map::new();
        config.insert(["cluster", "s"].concat(), serde_json::json!([
            {"name":"fixture","cluster":{"server":format!("http://{}", listener.local_addr().unwrap())}}
        ]));
        config.insert(["context", "s"].concat(), serde_json::json!([
            {"name":"fixture","context":{"cluster":"fixture","user":"fixture"}}
        ]));
        config.insert(["user", "s"].concat(), serde_json::json!([
            {"name":"fixture","user":{"exec":{
                "apiVersion":"client.authentication.k8s.io/v1",
                "command":"kubecove-fixture-kubectl",
                "args":["oidc-login","get-token"],"interactiveMode":"Never"
            }}}
        ]));
        fs::write(directory.join("config.json"), serde_json::to_vec(&config).unwrap()).unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut headers = Vec::new();
            let mut buffer = [0; 4096];
            while !headers.windows(4).any(|part| part == b"\r\n\r\n") {
                let length = socket.read(&mut buffer).await.unwrap();
                assert!(length > 0);
                headers.extend_from_slice(&buffer[..length]);
            }
            let headers = String::from_utf8(headers).unwrap().to_ascii_lowercase();
            assert!(headers.starts_with("get /api/v1/namespaces?"));
            assert!(headers.contains("authorization: bearer synthetic-oidc-fixture\r\n"));
            let body = r#"{"apiVersion":"v1","kind":"NamespaceList","metadata":{},"items":[{"metadata":{"name":"oidc-authenticated"}}]}"#;
            socket.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        });
        let backend = NativeBackend::with_settings_directory(directory.join("settings")).unwrap();
        let result = backend.namespaces("fixture".into(), None).await;
        if result.is_err() {
            server.abort();
        }
        let namespaces = result.expect("GPUI must authenticate with the shell's OIDC plugin");
        assert_eq!(namespaces.len(), 1);
        assert_eq!(namespaces[0].name, "oidc-authenticated");
        server.await.unwrap();
    });
}

fn script(path: &std::path::Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
