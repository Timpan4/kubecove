//! Real transport against two controlled loopback endpoints. No cluster or
//! external server is contacted and no credential is written or printed.
use futures_util::StreamExt;
use kubecove_gpui::backend::{Backend, DesktopBackend, NativeBackend, Scope, WatchTarget};
use kubecove_lib::models::{AppErrorKind, ArgoApplicationRef};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinSet,
};

type Requests = tokio::sync::mpsc::UnboundedReceiver<(String, serde_json::Value)>;

/// The saved sources a controlled server returns for one kubeconfig edit.
fn saved_sources(command: &str, arguments: &serde_json::Value) -> serde_json::Value {
    let env_var = if command == "set_kubeconfig_env_var" {
        arguments["envVar"].clone()
    } else {
        json!("KUBECONFIG")
    };
    let paths: Vec<_> = if command == "add_kubeconfig_paths" {
        arguments["paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|path| json!({"path": path}))
            .collect()
    } else {
        Vec::new()
    };
    json!({"kubeconfigEnvVar":env_var,"paths":paths,"sourceKey":"remote","sourceLabel":"Server","showSourceLabels":true,"warnings":[]})
}
async fn server(name: &'static str) -> (String, tokio::task::JoinHandle<()>, Requests) {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sent, requests) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        let mut connections = JoinSet::new();
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let sent = sent.clone();
            connections.spawn(async move {
                let mut first = [0];
                stream.peek(&mut first).await.unwrap();
                if first[0] == b'G' {
                    let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                    while socket.next().await.is_some() {}
                    return;
                }
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") { header.push(stream.read_u8().await.unwrap()); }
                let header = String::from_utf8(header).unwrap();
                let length = header.lines().find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(str::trim).and_then(|value| value.parse::<usize>().ok())).unwrap_or_default();
                let mut body = vec![0;length]; stream.read_exact(&mut body).await.unwrap();
                let command = header.lines().next().unwrap().split(' ').nth(1).unwrap().strip_prefix("/api/v1/invoke/");
                if let Some(command) = command { let _ = sent.send((command.to_owned(), serde_json::from_slice(&body).unwrap())); }
                let response = if header.starts_with("POST /api/v1/session ") {
                    // The session grace default is from ADR 0020. This test does not wait for it.
                    json!({"resumeToken":"controlled-session","sessionGraceSeconds":300,"serverVersion":"0.13.0"})
                } else if header.starts_with("POST /api/v1/invoke/list_kube_contexts ") {
                    json!([{"name":name,"isCurrent":true}])
                } else if matches!(command, Some("list_namespaces" | "list_resource_kinds" | "list_resources")) {
                    json!([])
                } else if command == Some("start_resource_watch") { json!("controlled-stream") }
                else if let Some(command @ ("set_kubeconfig_env_var" | "add_kubeconfig_paths")) = command {
                    saved_sources(command, &serde_json::from_slice(&body).unwrap())
                }
                else { json!(true) };
                let body = response.to_string();
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len());
                stream.write_all(response.as_bytes()).await.unwrap();
                stream.shutdown().await.unwrap();
            });
        }
    });
    (url, task, requests)
}

#[test]
fn switching_remote_endpoints_routes_reads_and_disconnect_clears_selection() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = DesktopBackend::new(NativeBackend::new().unwrap());
    runtime.block_on(async {
        let (first, first_task, _) = server("first-controlled-host").await;
        let (second, second_task, _) = server("second-controlled-host").await;
        let mut local = backend.load_preferences().unwrap();
        local.preferences.argo_connection_preferences.insert(
            "local-workspace".into(),
            kubecove_gpui::preferences::ConnectionPreference::Kubernetes,
        );
        backend.save_preferences(local.clone()).unwrap();
        backend
            .connect_remote(first, "controlled-test-credential".into())
            .await
            .unwrap();
        assert_eq!(
            backend.contexts().await.unwrap()[0].name,
            "first-controlled-host"
        );
        assert!(
            backend
                .load_preferences()
                .unwrap()
                .preferences
                .argo_connection_preferences
                .is_empty()
        );
        let mut remote = backend.load_preferences().unwrap();
        remote.preferences.argo_connection_preferences.insert(
            "remote-workspace".into(),
            kubecove_gpui::preferences::ConnectionPreference::Kubernetes,
        );
        backend.save_preferences(remote).unwrap();
        backend
            .connect_remote(second, "controlled-test-credential".into())
            .await
            .unwrap();
        assert_eq!(
            backend.contexts().await.unwrap()[0].name,
            "second-controlled-host"
        );
        assert!(
            backend
                .load_preferences()
                .unwrap()
                .preferences
                .argo_connection_preferences
                .is_empty()
        );
        backend.disconnect_remote().await.unwrap();
        assert_eq!(
            backend.load_preferences().unwrap().preferences,
            local.preferences
        );
        assert!(backend.remote_status().await.unwrap().is_none());
        assert!(
            !backend
                .kubeconfig_writes_unavailable()
                .unwrap()
                .contains("server operator")
        );
        first_task.abort();
        second_task.abort();
        let _ = first_task.await;
        let _ = second_task.await;
    });
}

#[test]
fn kubeconfig_env_var_and_server_path_save_on_the_connected_server() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = DesktopBackend::new(NativeBackend::new().unwrap());
    runtime.block_on(async {
        let (url, task, mut requests) = server("remote-kubeconfig-host").await;
        backend
            .connect_remote(url, "controlled-test-credential".into())
            .await
            .unwrap();
        assert!(backend.kubeconfig_source_edits_unavailable().is_none());
        assert!(backend.kubeconfig_writes_unavailable().is_some());
        let sources = backend
            .set_kubeconfig_env_var("PARITY_REMOTE".into())
            .await
            .unwrap();
        assert_eq!(sources.kubeconfig_env_var, "PARITY_REMOTE");
        let sources = backend
            .add_kubeconfig_paths(vec!["/srv/remote.yaml".into()])
            .await
            .unwrap();
        assert_eq!(sources.paths[0].path, "/srv/remote.yaml");
        let mut sent = Vec::new();
        while let Ok(request) = requests.try_recv() {
            sent.push(request);
        }
        assert!(sent.contains(&(
            "set_kubeconfig_env_var".into(),
            json!({"envVar": "PARITY_REMOTE"})
        )));
        assert!(sent.contains(&(
            "add_kubeconfig_paths".into(),
            json!({"paths": ["/srv/remote.yaml"]})
        )));
        task.abort();
        let _ = task.await;
    });
}

#[test]
fn remote_delivery_dynamic_refresh_and_stream_requests_follow_the_shared_contract() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = DesktopBackend::new(NativeBackend::new().unwrap());
    runtime.block_on(async {
        let (url, task, mut requests) = server("controlled-host").await;
        backend.connect_remote(url, "controlled-test-credential".into()).await.unwrap();
        let release = serde_json::from_value(json!({"cluster":"controlled-host","name":"release","namespace":"payments","age":"1d","storageKind":"Secret","storageName":"sh.helm.release.v1.release.v1"})).unwrap();
        assert_eq!(backend.helm_release(release).await.unwrap_err().kind, AppErrorKind::Serialization);
        let (command, args) = requests.recv().await.unwrap();
        assert_eq!(command, "get_helm_release_details");
        assert_eq!(args["storageKind"], "Secret");
        let application = ArgoApplicationRef { name: "payments".into(), context: Some("controlled-host".into()), ..Default::default() };
        assert_eq!(backend.argo_inspector(application).await.unwrap_err().kind, AppErrorKind::Serialization);
        let (_, args) = requests.recv().await.unwrap();
        assert_eq!(args["clusterContext"], "controlled-host");
        assert_eq!(backend.deployment_revisions(super::resource("controlled-host")).await.unwrap_err().kind, AppErrorKind::Serialization);
        let (command, args) = requests.recv().await.unwrap();
        assert_eq!(command, "list_deployment_revisions");
        assert!(args.get("kind").is_none());
        let mut dynamic = super::resource("controlled-host");
        dynamic.dynamic = Some(true);
        dynamic.api_version = Some("apps/v1".into());
        dynamic.group = Some("apps".into());
        dynamic.version = Some("v1".into());
        dynamic.plural = Some("deployments".into());
        assert_eq!(backend.details(dynamic).await.unwrap_err().kind, AppErrorKind::Serialization);
        let (command, args) = requests.recv().await.unwrap();
        assert_eq!(command, "get_dynamic_resource_details");
        assert!(args.get("kind").is_none());
        let scope = Scope { context: "controlled-host".into(), namespace: Some("payments".into()) };
        backend.resources(scope.clone(), true).await.unwrap();
        let (command, _) = requests.recv().await.unwrap();
        assert_eq!(command, "list_resource_kinds");
        let (command, args) = requests.recv().await.unwrap();
        assert_eq!(command, "refresh_resource_cache");
        assert!(!args["keys"].as_array().unwrap().is_empty());
        assert_eq!(args["namespaces"], json!(["payments"]));
        let key = serde_json::from_value(json!({"resourceKind":{"kind":"Deployment","namespaced":true},"namespace":"payments"})).unwrap();
        let stream = backend.watch(WatchTarget::Resource { scope, key }).await.unwrap();
        drop(stream);
        loop {
            let (command, args) = requests.recv().await.unwrap();
            if command == "stop_stream" {
                assert_eq!(args["streamId"], "controlled-stream");
                break;
            }
        }
        backend.disconnect_remote().await.unwrap();
        task.abort();
        let _ = task.await;
    });
}
