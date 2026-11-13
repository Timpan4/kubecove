//! Failure modes beyond the in-memory parity seam. All files belong to this worktree.
use kubecove_gpui::backend::{Backend, NativeBackend};
use std::{
    net::TcpListener,
    path::PathBuf,
    time::{Duration, Instant},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.t/mcp-tmp")
            .join(format!("host-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let path = path.join(format!("{:?}", std::thread::current().id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn backend(&self) -> NativeBackend {
        NativeBackend::new()
            .unwrap()
            .with_mcp_directory(self.0.clone())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

/// A dropped host shuts its runtime down in the background, so its listening socket closes
/// shortly after the drop returns. The next host binds once at construction and reports a
/// port in use instead of retrying, so a test that reopens on the same port waits here.
/// Five seconds is the deadline this workspace's other local waits use.
fn wait_until_free(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while TcpListener::bind(("127.0.0.1", port)).is_err() {
        assert!(
            Instant::now() < deadline,
            "port {port} stayed bound after its host was dropped"
        );
        std::thread::yield_now();
    }
}

#[test]
fn port_in_use_is_visible_and_disabling_recovers() {
    let directory = Directory::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = occupied.local_addr().unwrap().port();
    let backend = directory.backend();
    let status = runtime
        .block_on(backend.set_mcp_settings(true, port))
        .unwrap();
    assert!(status.enabled && !status.running);
    assert!(status.error.unwrap().contains("Could not listen on"));
    let status = runtime
        .block_on(backend.set_mcp_settings(false, port))
        .unwrap();
    assert!(!status.enabled && !status.running && status.error.is_none());
}

#[test]
fn token_file_failure_stops_startup_without_hiding_the_error() {
    let directory = Directory::new();
    std::fs::create_dir(directory.0.join("mcp-token")).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = directory.backend();
    // Token loading fails before any bind.
    let status = runtime
        .block_on(backend.set_mcp_settings(true, 47615))
        .unwrap();
    assert!(status.enabled && !status.running);
    assert!(status.error.unwrap().contains("MCP token"));
    assert!(runtime.block_on(backend.reveal_mcp_token()).is_err());
    assert!(runtime.block_on(backend.rotate_mcp_token()).is_err());
}

#[test]
fn token_rotation_and_settings_survive_backend_recreation() {
    let directory = Directory::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = directory.backend();
    let original = runtime.block_on(backend.reveal_mcp_token()).unwrap();
    assert!(runtime.block_on(backend.reveal_mcp_token()).unwrap() == original);
    runtime.block_on(backend.rotate_mcp_token()).unwrap();
    let rotated = runtime.block_on(backend.reveal_mcp_token()).unwrap();
    assert!(original != rotated, "rotation must replace the credential");
    runtime
        .block_on(backend.set_mcp_settings(false, 47616))
        .unwrap();
    assert!(!directory.0.join("live-sessions.json").exists());
    let settings = std::fs::read_to_string(directory.0.join("mcp-settings.json")).unwrap();
    assert!(!settings.contains(&original) && !settings.contains(&rotated));
    drop(backend);
    let reopened = directory.backend();
    let status = runtime.block_on(reopened.mcp_status()).unwrap();
    assert!(!status.enabled && status.port == 47616);
    assert!(runtime.block_on(reopened.reveal_mcp_token()).unwrap() == rotated);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(directory.0.join("mcp-token"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn running_rotation_revokes_old_token_and_restart_reuses_saved_settings() {
    let directory = Directory::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let reserved = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let backend = directory.backend();
    let status = runtime
        .block_on(backend.set_mcp_settings(true, port))
        .unwrap();
    assert!(status.running, "test-chosen loopback listener must start");
    let url = status.url.unwrap();
    let original = runtime.block_on(backend.reveal_mcp_token()).unwrap();
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let request = |token: String| {
        client
            .post(&url)
            .bearer_auth(token)
            .header("accept", "application/json, text/event-stream")
            .json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .send()
    };
    assert!(
        runtime
            .block_on(request(original.clone()))
            .unwrap()
            .status()
            .is_success()
    );
    assert!(
        runtime
            .block_on(backend.rotate_mcp_token())
            .unwrap()
            .running
    );
    let rotated = runtime.block_on(backend.reveal_mcp_token()).unwrap();
    assert_eq!(
        runtime.block_on(request(original)).unwrap().status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let response = runtime.block_on(request(rotated.clone())).unwrap();
    assert!(response.status().is_success());
    let body: serde_json::Value = runtime.block_on(response.json()).unwrap();
    let tools = body["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 11);
    assert!(
        tools
            .iter()
            .all(|tool| tool["annotations"]["readOnlyHint"] == true)
    );
    // Dropping the native host closes its listener. The next host restores the same port.
    drop(backend);
    wait_until_free(port);
    let reopened = directory.backend();
    let status = runtime.block_on(reopened.mcp_status()).unwrap();
    assert!(status.running && status.port == port);
    assert!(runtime.block_on(reopened.reveal_mcp_token()).unwrap() == rotated);
    runtime
        .block_on(reopened.set_mcp_settings(false, port))
        .unwrap();
    let closed = TcpListener::bind(("127.0.0.1", port)).unwrap();
    drop(closed);
}

#[test]
fn unreadable_or_malformed_settings_preserve_the_original() {
    let directory = Directory::new();
    let path = directory.0.join("mcp-settings.json");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for content in ["{", r#"{"enabled":true,"port":0}"#] {
        std::fs::write(&path, content).unwrap();
        let backend = directory.backend();
        assert!(runtime.block_on(backend.mcp_status()).is_err());
        assert!(
            runtime
                .block_on(backend.set_mcp_settings(false, 47615))
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let backend = directory.backend();
    assert!(runtime.block_on(backend.mcp_status()).is_err());
    assert!(
        runtime
            .block_on(backend.set_mcp_settings(false, 47615))
            .is_err()
    );
    assert!(path.is_dir());
}

#[test]
fn failed_save_keeps_the_previous_settings_and_runtime_state() {
    let directory = Directory::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = directory.backend();
    runtime
        .block_on(backend.set_mcp_settings(false, 47616))
        .unwrap();
    let path = directory.0.join("mcp-settings.json");
    let original = std::fs::read(&path).unwrap();
    std::fs::create_dir(directory.0.join("mcp-settings.json.tmp")).unwrap();
    assert!(
        runtime
            .block_on(backend.set_mcp_settings(true, 47615))
            .is_err()
    );
    let status = runtime.block_on(backend.mcp_status()).unwrap();
    assert!(!status.enabled && !status.running && status.port == 47616);
    assert_eq!(std::fs::read(path).unwrap(), original);
}
