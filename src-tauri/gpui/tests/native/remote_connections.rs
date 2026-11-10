//! Real transport against two controlled loopback endpoints. No cluster or
//! external server is contacted and no credential is written or printed.
use futures_util::StreamExt;
use kubecove_gpui::backend::{Backend, DesktopBackend, NativeBackend, Scope, WatchTarget};
use kubecove_lib::models::{AppErrorKind, ArgoApplicationRef, WatchResourceKey, WatchResourceKind};
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
    // Add and reorder return the paths the client sent. Remove returns none, which is enough
    // for these controlled responses.
    let paths: Vec<_> = match command {
        "add_kubeconfig_paths" | "reorder_kubeconfig_paths" => arguments["paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|path| json!({"path": path}))
            .collect(),
        _ => Vec::new(),
    };
    let show = if command == "set_show_kubeconfig_source_labels" {
        arguments["show"].clone()
    } else {
        json!(true)
    };
    json!({"kubeconfigEnvVar":env_var,"paths":paths,"sourceKey":"remote","sourceLabel":"Server","showSourceLabels":show,"warnings":[]})
}
/// Counts PersistentVolume lists, so each one reports a newer snapshot.
static VOLUME_LISTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The controlled `list_resource_scope` answer: no rows, a version of 4242 for Pod lists, and
/// `pv-<n>` for the nth PersistentVolume list. Other kinds come back without a version, as a
/// list the server did not version.
fn scope_result(arguments: &serde_json::Value) -> serde_json::Value {
    let requests = arguments["requests"].as_array().unwrap();
    let resource_versions: Vec<_> = requests
        .iter()
        .filter_map(|request| match request["kind"].as_str() {
            Some("Pod") => Some(json!({"request": request, "resourceVersion": "4242"})),
            Some("PersistentVolume") => {
                let list = VOLUME_LISTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                Some(json!({"request": request, "resourceVersion": format!("pv-{list}")}))
            }
            _ => None,
        })
        .collect();
    json!({"resources":[],"completedRequests":requests,"resourceVersions":resource_versions,"failures":[]})
}
/// Every list on `same-kind-host` returns two custom resources that share kind, name and
/// namespace but come from different API groups.
fn same_kind_rows(cluster: &str) -> serde_json::Value {
    let rows = ["a.example.com", "b.example.com"].map(|group| {
        json!({"kind":"Widget","group":group,"cluster":cluster,"name":"shared","namespace":"payments","age":"1d"})
    });
    json!({"resources":rows,"completedRequests":[],"resourceVersions":[],"failures":[]})
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
                else if command == Some("list_resource_scope") && name == "same-kind-host" { same_kind_rows(name) }
                else if command == Some("list_resource_scope") { scope_result(&serde_json::from_slice(&body).unwrap()) }
                else if matches!(command, Some("list_argocd_applications" | "list_argocd_appsets" | "list_argocd_appprojects")) { json!({"items": []}) }
                else if command == Some("detect_flux") { json!({"detected": false, "kinds": [], "missingKinds": []}) }
                else if let Some(command @ ("set_kubeconfig_env_var"| "add_kubeconfig_paths" | "remove_kubeconfig_path" | "reorder_kubeconfig_paths" | "set_show_kubeconfig_source_labels")) = command {
                    saved_sources(command, &serde_json::from_slice(&body).unwrap())
                }
                else { json!(true) };
                // A server that revoked the sign-in refuses the next read, and one whose
                // user may not list ApplicationSets refuses that list alone.
                let (status, response) = if name == "revoking-host" && command == Some("list_resource_kinds") {
                    ("401 Unauthorized", json!({"message":"Sign in again.","kind":"unauthenticated"}))
                } else if name == "gitops-host" && command == Some("list_argocd_appsets") {
                    ("403 Forbidden", json!({"message":"ApplicationSets are forbidden.","kind":"forbidden"}))
                } else { ("200 OK", response) };
                let body = response.to_string();
                let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len());
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
            backend.contexts(None).await.unwrap()[0].name,
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
            backend.contexts(None).await.unwrap()[0].name,
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
                .contains("Remote mode")
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
        let sources = backend
            .reorder_kubeconfig_paths(vec!["/srv/second.yaml".into(), "/srv/remote.yaml".into()])
            .await
            .unwrap();
        assert_eq!(sources.paths[0].path, "/srv/second.yaml");
        let sources = backend
            .set_show_kubeconfig_source_labels(false)
            .await
            .unwrap();
        assert!(!sources.show_source_labels);
        backend
            .remove_kubeconfig_path("/srv/second.yaml".into())
            .await
            .unwrap();
        let mut sent = Vec::new();
        while let Ok(request) = requests.try_recv() {
            sent.push(request);
        }
        assert!(sent.contains(&(
            "reorder_kubeconfig_paths".into(),
            json!({"paths": ["/srv/second.yaml", "/srv/remote.yaml"]})
        )));
        assert!(sent.contains(&(
            "set_show_kubeconfig_source_labels".into(),
            json!({"show": false})
        )));
        assert!(sent.contains(&(
            "remove_kubeconfig_path".into(),
            json!({"path": "/srv/second.yaml"})
        )));
        task.abort();
        let _ = task.await;
    });
}

#[test]
fn remote_reads_carry_list_versions_to_watch_starts() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = DesktopBackend::new(NativeBackend::new().unwrap());
    runtime.block_on(async {
        let (url, task, mut requests) = server("controlled-host").await;
        backend
            .connect_remote(url, "controlled-test-credential".into())
            .await
            .unwrap();
        let scope = Scope {
            context: "controlled-host".into(),
            namespace: Some("payments".into()),
        };
        let kind = |name: &str| WatchResourceKey {
            resource_kind: WatchResourceKind {
                kind: name.into(),
                group: None,
                version: None,
                api_version: None,
                plural: None,
                namespaced: Some(true),
            },
            namespace: Some("payments".into()),
        };
        let scope_read = backend.resources(scope.clone(), false).await.unwrap();
        let versions: Vec<_> = scope_read.versions.values().map(String::as_str).collect();
        assert_eq!(
            versions,
            vec!["4242"],
            "the scope read must keep the Pod list version"
        );
        let pods = backend
            .kind_resources(scope.clone(), kind("Pod"), Vec::new())
            .await
            .unwrap();
        assert_eq!(pods.resource_version.as_deref(), Some("4242"));
        let deployments = backend
            .kind_resources(scope.clone(), kind("Deployment"), Vec::new())
            .await
            .unwrap();
        assert_eq!(
            deployments.resource_version, None,
            "a list without a version must not get one"
        );
        let stream = backend
            .watch(WatchTarget::Resource {
                scope,
                key: Box::new(kind("Pod")),
                resource_version: pods.resource_version,
            })
            .await
            .unwrap();
        drop(stream);
        loop {
            let (command, args) = requests.recv().await.unwrap();
            if command == "start_resource_watch" {
                assert_eq!(args["keys"][0]["resourceVersion"], "4242");
                break;
            }
        }
        backend.disconnect_remote().await.unwrap();
        task.abort();
        let _ = task.await;
    });
}

/// An Application across two namespaces lists each cluster-scoped kind once per namespace.
/// The rows kept for that kind come from the first list, so its watch must start at the first
/// list's version: a later version would skip a deletion between the two lists and leave a
/// stale row.
#[test]
fn remote_application_read_keeps_the_first_cluster_list_version() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = DesktopBackend::new(NativeBackend::new().unwrap());
    runtime.block_on(async {
        let (url, task, _requests) = server("controlled-host").await;
        backend
            .connect_remote(url, "controlled-test-credential".into())
            .await
            .unwrap();
        let application = serde_json::from_value(json!({
            "cluster": "controlled-host",
            "name": "spread",
            "age": "1d",
            "healthAssessment": kubecove_lib::models::HealthAssessment::default(),
            "sources": [],
            "resourceNamespaces": ["payments", "shop"],
        }))
        .unwrap();
        let before = VOLUME_LISTS.load(std::sync::atomic::Ordering::SeqCst);
        let read = backend
            .application_resources(application, false)
            .await
            .unwrap();
        let first = format!("pv-{}", before + 1);
        let volumes = kubecove_gpui::backend::watch_signature(&WatchResourceKey {
            resource_kind: WatchResourceKind {
                kind: "PersistentVolume".into(),
                group: None,
                version: None,
                api_version: None,
                plural: None,
                namespaced: Some(false),
            },
            namespace: None,
        });
        assert_eq!(
            read.versions.get(&volumes),
            Some(&first),
            "the PersistentVolume watch must start at the first list's version"
        );
        backend.disconnect_remote().await.unwrap();
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
        let stream = backend.watch(WatchTarget::Resource { scope, key: Box::new(key), resource_version: Some("7".into()) }).await.unwrap();
        drop(stream);
        loop {
            let (command, args) = requests.recv().await.unwrap();
            if command == "start_resource_watch" {
                assert_eq!(args["keys"][0]["resourceVersion"], "7");
            }
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

/// The app runs on `DesktopBackend`; without a remote its session controls are this host's.
#[test]
fn local_session_controls_reach_this_host() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = DesktopBackend::new(NativeBackend::new().unwrap());
    runtime.block_on(async {
        let cleaned = backend
            .cleanup_sessions(kubecove_lib::models::LiveSessionCleanupRequest {
                allowed_cluster_contexts: vec![],
                kubeconfig_source_key: "kubeconfigSource=active".into(),
            })
            .await
            .unwrap();
        assert_eq!(cleaned.stopped_port_forwards, 0);
        let stopped = backend
            .stop_forward("no-such-forward".into())
            .await
            .unwrap();
        assert!(!stopped);
    });
}

#[test]
fn a_sign_in_revoked_during_a_read_ends_the_session_without_local_fallback() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = DesktopBackend::new(NativeBackend::new().unwrap());
    runtime.block_on(async {
        let (url, task, _) = server("revoking-host").await;
        backend
            .connect_remote(url, "controlled-test-credential".into())
            .await
            .unwrap();
        let mut changes = backend.remote_session_changes();
        changes.borrow_and_update();
        let error = backend
            .resource_kinds("revoking-host".into())
            .await
            .unwrap_err();
        assert!(!error.message.contains("controlled-test-credential"));
        // The inspector reads the status again when the session signal fires.
        assert!(changes.has_changed().unwrap());
        let status = backend.remote_status().await.unwrap().unwrap();
        assert!(!status.connected);
        assert_eq!(
            status.session_state,
            kubecove_host::remote::RemoteSessionState::SignInExpired
        );
        assert!(status.notice().is_some());
        // Later reads fail instead of reaching this laptop's kubeconfig.
        let error = backend.contexts(None).await.unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Session);
        backend.disconnect_remote().await.unwrap();
        task.abort();
        let _ = task.await;
    });
}

#[test]
fn remote_application_read_keeps_same_kind_resources_from_two_api_groups() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = DesktopBackend::new(NativeBackend::new().unwrap());
    runtime.block_on(async {
        let (url, task, _) = server("same-kind-host").await;
        backend
            .connect_remote(url, "controlled-test-credential".into())
            .await
            .unwrap();
        let application = serde_json::from_value(json!({
            "cluster": "same-kind-host",
            "name": "widgets",
            "age": "1d",
            "healthAssessment": kubecove_lib::models::HealthAssessment::default(),
            "sources": [],
            "resourceNamespaces": ["payments", "shop"],
        }))
        .unwrap();
        let read = backend
            .application_resources(application, false)
            .await
            .unwrap();
        let mut groups: Vec<_> = read.resources.iter().map(|row| row.group.clone()).collect();
        groups.sort();
        assert_eq!(
            groups,
            [
                Some("a.example.com".to_owned()),
                Some("b.example.com".to_owned())
            ]
        );
        backend.disconnect_remote().await.unwrap();
        task.abort();
        let _ = task.await;
    });
}

#[test]
fn a_forbidden_gitops_list_keeps_the_rest_of_the_remote_inventory() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let backend = DesktopBackend::new(NativeBackend::new().unwrap());
    runtime.block_on(async {
        let (url, task, _) = server("gitops-host").await;
        backend
            .connect_remote(url, "controlled-test-credential".into())
            .await
            .unwrap();
        let inventory = backend.gitops("gitops-host".into()).await.unwrap();
        assert!(inventory.argo.as_ref().is_some_and(Vec::is_empty));
        let error = inventory.list_error.expect("the refused list is reported");
        assert_eq!(error.kind, AppErrorKind::Forbidden);
        assert!(inventory.flux.is_none());
        backend.disconnect_remote().await.unwrap();
        task.abort();
        let _ = task.await;
    });
}
