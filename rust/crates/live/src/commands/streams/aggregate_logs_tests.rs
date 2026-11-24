use super::*;
use http::{Request, Response};
use kube::client::Body;
use serde_json::json;

// Failure mode: source errors always use LogLines, so clients that omit or
// disable batchLines never see the error. Preserve the source in both modes.
#[tokio::test]
async fn source_errors_honor_batch_lines_opt_in() {
    use std::sync::{Arc, Mutex};

    for flag in [None, Some(false), Some(true)] {
        let mut request = serde_json::json!({
            "clusterContext": "test", "namespace": "default",
            "targetKind": "Deployment", "targetName": "api"
        });
        if let Some(flag) = flag {
            request["batchLines"] = flag.into();
        }
        let request: AggregatedLogStreamRequest = serde_json::from_value(request).unwrap();
        let received = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let sink = Arc::clone(&received);
        let channel = Channel::new(move |message| {
            let message = serde_json::to_value(message).unwrap();
            sink.lock().unwrap().push(message);
            Ok(())
        });
        let (service, mut handle) = tower_test::mock::pair::<
            http::Request<kube::client::Body>,
            http::Response<kube::client::Body>,
        >();
        let stream = stream_log_source(
            Client::new(service, "default"),
            "default".into(),
            LogSourceKey {
                pod_name: "api".into(),
                container: "app".into(),
            },
            LogStreamOptions {
                batch_lines: request.batch_lines,
                tail_lines: None,
                since_seconds: None,
            },
            channel,
            "logs".into(),
        );
        let respond = async {
            let (_, send) = handle.next_request().await.unwrap();
            send.send_response(http::Response::builder().status(403).body(kube::client::Body::from(
                br#"{"apiVersion":"v1","kind":"Status","status":"Failure","message":"denied","reason":"Forbidden","code":403}"#.to_vec(),
            )).unwrap());
        };
        tokio::join!(stream, respond);
        let messages = received.lock().unwrap();
        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message["streamId"], "logs");
        assert_eq!(
            message["source"],
            serde_json::json!({"podName": "api", "container": "app"})
        );
        let line = if request.batch_lines {
            assert_eq!(message["type"], "logLines");
            assert_eq!(message["lines"].as_array().unwrap().len(), 1);
            &message["lines"][0]
        } else {
            assert_eq!(message["type"], "logLine");
            &message["line"]
        };
        assert!(line.as_str().unwrap().starts_with("log stream error: "));
        assert!(line.as_str().unwrap().contains("denied"));
    }
}

use k8s_openapi::{
    api::core::v1::{Container, PodSpec, ServiceSpec},
    apimachinery::pkg::apis::meta::v1::{LabelSelectorRequirement, Time},
};
use kube::core::ObjectMeta;

fn pod(name: &str, containers: &[&str]) -> Pod {
    Pod {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            ..Default::default()
        },
        spec: Some(PodSpec {
            containers: containers
                .iter()
                .map(|name| Container {
                    name: (*name).to_string(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn deployment_selector_supports_match_labels_and_expressions() {
    let selector = LabelSelector {
        match_labels: Some(BTreeMap::from([(
            "app.kubernetes.io/name".to_string(),
            "api".to_string(),
        )])),
        match_expressions: Some(vec![
            LabelSelectorRequirement {
                key: "tier".to_string(),
                operator: "In".to_string(),
                values: Some(vec!["web".to_string(), "worker".to_string()]),
            },
            LabelSelectorRequirement {
                key: "canary".to_string(),
                operator: "DoesNotExist".to_string(),
                values: None,
            },
        ]),
    };

    assert_eq!(
        selector_from_label_selector(&selector).expect("selector"),
        "app.kubernetes.io/name=api,tier in (web,worker),!canary",
    );
}

#[test]
fn deployment_selector_rejects_empty_or_invalid_expressions() {
    assert_eq!(
        selector_from_label_selector(&LabelSelector::default())
            .expect_err("empty selector")
            .message,
        "Deployment selector is empty",
    );
    let selector = LabelSelector {
        match_labels: None,
        match_expressions: Some(vec![LabelSelectorRequirement {
            key: "tier".to_string(),
            operator: "In".to_string(),
            values: None,
        }]),
    };
    assert_eq!(
        selector_from_label_selector(&selector)
            .expect_err("missing values")
            .message,
        "Deployment selector In expression needs values",
    );
}

#[test]
fn service_selector_requires_selector_backed_service() {
    let selector = BTreeMap::from([
        ("app".to_string(), "api".to_string()),
        ("tier".to_string(), "web".to_string()),
    ]);
    let service = Service {
        spec: Some(ServiceSpec {
            selector: Some(selector),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        service_label_selector(&service).expect("selector"),
        "app=api,tier=web",
    );

    let empty = Service {
        spec: Some(ServiceSpec {
            selector: None,
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        service_label_selector(&empty)
            .expect_err("selectorless")
            .kind,
        AppErrorKind::Validation,
    );
}

#[test]
fn log_sources_skip_deleting_pods_and_cap_results() {
    let mut deleting = pod("api-old", &["app"]);
    deleting.metadata.deletion_timestamp =
        Some(Time(k8s_openapi::jiff::Timestamp::from_second(0).unwrap()));
    let pods = std::iter::once(deleting)
        .chain((0..60).map(|index| pod(&format!("api-{index:02}"), &["app"])))
        .collect::<Vec<_>>();

    let index = PodSources::from_pods(&pods);
    let sources = index.selected();

    assert_eq!(index.total, 60);
    assert_eq!(sources.len(), MAX_AGGREGATED_LOG_SOURCES);
    assert_eq!(sources[0].pod_name, "api-00");
    assert!(sources.iter().all(|source| source.container == "app"));
}

#[tokio::test(start_paused = true)]
async fn pod_updates_keep_the_watch_open_and_expiration_relists() {
    let (service, mut requests) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    let client = Client::new(service, "default");
    let pods = Api::namespaced(client.clone(), "default");
    let mut logs = AggregateLogs {
        client,
        namespace: "default".into(),
        options: LogStreamOptions {
            batch_lines: false,
            tail_lines: Some(10),
            since_seconds: None,
        },
        channel: Channel::new(|_| Ok(())),
        stream_id: "logs".into(),
        streams: SourceStreams::default(),
        pods: PodSources::default(),
        selected: Vec::new(),
    };

    let mock = async {
        let (list, reply) = requests.next_request().await.unwrap();
        assert_eq!(list.uri().path(), "/api/v1/namespaces/default/pods");
        assert!(!list.uri().query().unwrap().contains("watch=true"));
        let mut first = pod("api-a", &[]);
        first.metadata.resource_version = Some("10".into());
        reply.send_response(Response::new(Body::from(
            json!({"metadata": {"resourceVersion": "10"}, "items": [&first]})
                .to_string()
                .into_bytes(),
        )));

        let (watch, reply) = requests.next_request().await.unwrap();
        let query = watch.uri().query().unwrap();
        assert!(query.contains("watch=true"));
        assert!(query.contains("resourceVersion=10"));
        first.metadata.resource_version = Some("11".into());
        let mut second = pod("api-b", &["app"]);
        second.metadata.resource_version = Some("12".into());
        reply.send_response(Response::new(Body::from(
            format!(
                "{}\n{}\n",
                json!({"type": "MODIFIED", "object": first}),
                json!({"type": "ADDED", "object": second}),
            )
            .into_bytes(),
        )));

        let (log, _log_response) = requests.next_request().await.unwrap();
        assert_eq!(
            log.uri().path(),
            "/api/v1/namespaces/default/pods/api-b/log"
        );
        assert!(log.uri().query().unwrap().contains("tailLines=10"));

        let (watch, reply) = requests.next_request().await.unwrap();
        let query = watch.uri().query().unwrap();
        assert!(query.contains("watch=true"));
        assert!(query.contains("resourceVersion=12"));
        reply.send_response(Response::new(Body::from(
            format!(
                "{}\n",
                json!({"type": "ERROR", "object": {
                    "apiVersion": "v1", "kind": "Status", "status": "Failure",
                    "reason": "Expired", "message": "old watch cursor", "code": 410,
                }})
            )
            .into_bytes(),
        )));

        let (list, reply) = requests.next_request().await.unwrap();
        assert_eq!(list.uri().path(), "/api/v1/namespaces/default/pods");
        assert!(!list.uri().query().unwrap().contains("watch=true"));
        reply.send_response(Response::new(Body::from(
            json!({"metadata": {"resourceVersion": "20"}, "items": [second]})
                .to_string()
                .into_bytes(),
        )));

        let (watch, _reply) = requests.next_request().await.unwrap();
        let query = watch.uri().query().unwrap();
        assert!(query.contains("watch=true"));
        assert!(query.contains("resourceVersion=20"));
    };

    tokio::select! {
        () = logs.watch_pods(&pods, "app=api") => panic!("log stream ended before the watch resumed"),
        () = mock => {},
    }
}

#[tokio::test]
async fn departed_log_sources_release_replay_bookkeeping() {
    let source = LogSourceKey {
        pod_name: "old-pod".into(),
        container: "app".into(),
    };
    let mut streams = SourceStreams::default();
    streams.attached.insert(source);
    let (service, _handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    let mut logs = AggregateLogs {
        client: Client::new(service, "default"),
        namespace: "default".into(),
        options: LogStreamOptions {
            batch_lines: false,
            tail_lines: Some(10),
            since_seconds: None,
        },
        channel: Channel::new(|_| Ok(())),
        stream_id: "logs".into(),
        streams,
        pods: PodSources::default(),
        selected: Vec::new(),
    };
    logs.reconcile_sources();
    assert!(logs.streams.attached.is_empty());
}

// Failure mode: a source whose log GET ends at once (a waiting, crash-looping or completed
// container) reconnects every time its task finishes, so an open watch sends one log GET and
// one error line per second per source. It must wait for a pod event or the watch renewal.
#[tokio::test(start_paused = true)]
async fn finished_sources_wait_for_a_pod_event_before_reconnecting() {
    use bytes::Bytes;
    use http_body::Frame;
    use http_body_util::{BodyExt, Full, StreamBody, combinators::UnsyncBoxBody};
    use std::convert::Infallible;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    type MockBody = UnsyncBoxBody<Bytes, Infallible>;
    let full = |body: String| Full::new(Bytes::from(body)).boxed_unsync();
    let (service, mut requests) = tower_test::mock::pair::<Request<Body>, Response<MockBody>>();
    let client = Client::new(service, "default");
    let pods = Api::namespaced(client.clone(), "default");
    let mut logs = AggregateLogs {
        client,
        namespace: "default".into(),
        options: LogStreamOptions {
            batch_lines: false,
            tail_lines: Some(10),
            since_seconds: None,
        },
        channel: Channel::new(|_| Ok(())),
        stream_id: "logs".into(),
        streams: SourceStreams::default(),
        pods: PodSources::default(),
        selected: Vec::new(),
    };

    let mut waiting = pod("api-a", &["app"]);
    waiting.metadata.resource_version = Some("10".into());
    let log_requests = Arc::new(AtomicUsize::new(0));
    let (events, receiver) = tokio::sync::mpsc::unbounded_channel::<Bytes>();
    let mut receiver = Some(receiver);
    let counted = Arc::clone(&log_requests);
    let listed = waiting.clone();
    let server = tokio::spawn(async move {
        while let Some((request, reply)) = requests.next_request().await {
            let query = request.uri().query().unwrap_or_default().to_owned();
            if request.uri().path().ends_with("/log") {
                counted.fetch_add(1, Ordering::SeqCst);
                let status = json!({
                    "apiVersion": "v1", "kind": "Status", "status": "Failure",
                    "reason": "BadRequest", "code": 400,
                    "message": "container app is waiting to start: CrashLoopBackOff",
                });
                reply.send_response(
                    Response::builder()
                        .status(400)
                        .body(full(status.to_string()))
                        .unwrap(),
                );
            } else if query.contains("watch=true") {
                // The watch stays open and carries only the events the test sends.
                let receiver = receiver.take().expect("the watch is not renewed");
                let frames = futures_util::stream::unfold(receiver, |mut receiver| async {
                    let bytes = receiver.recv().await?;
                    Some((Ok::<_, Infallible>(Frame::data(bytes)), receiver))
                });
                reply.send_response(Response::new(StreamBody::new(frames).boxed_unsync()));
            } else {
                reply.send_response(Response::new(full(
                    json!({"metadata": {"resourceVersion": "10"}, "items": [&listed]}).to_string(),
                )));
            }
        }
    });

    let check = async {
        tokio::time::sleep(Duration::from_secs(30)).await;
        assert_eq!(
            log_requests.load(Ordering::SeqCst),
            1,
            "a finished source reconnected without a pod event"
        );
        // A status-only change keeps the container set but can make the log readable again.
        let mut modify = |version: &str| {
            waiting.metadata.resource_version = Some(version.into());
            events
                .send(Bytes::from(format!(
                    "{}\n",
                    json!({"type": "MODIFIED", "object": waiting})
                )))
                .unwrap();
        };
        modify("11");
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(
            log_requests.load(Ordering::SeqCst),
            2,
            "a pod event retries the finished source once"
        );
        // An update during the retried task's cooldown must not be lost when it ends.
        modify("12");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(log_requests.load(Ordering::SeqCst), 3);
        modify("13");
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(
            log_requests.load(Ordering::SeqCst),
            4,
            "an update during the cooldown retries once the cooldown ends"
        );
    };
    tokio::select! {
        () = logs.watch_pods(&pods, "app=api") => panic!("the log stream ended"),
        () = check => {},
    }
    server.abort();
}

// Failure mode: RBAC allows listing pods but denies watching them. Keeping the cursor
// retries only the forbidden watch, so new pods are never found. A denial relists.
#[tokio::test(start_paused = true)]
async fn a_denied_watch_falls_back_to_listing() {
    let (service, mut requests) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    let client = Client::new(service, "default");
    let pods = Api::namespaced(client.clone(), "default");
    let mut logs = AggregateLogs {
        client,
        namespace: "default".into(),
        options: LogStreamOptions {
            batch_lines: false,
            tail_lines: Some(10),
            since_seconds: None,
        },
        channel: Channel::new(|_| Ok(())),
        stream_id: "logs".into(),
        streams: SourceStreams::default(),
        pods: PodSources::default(),
        selected: Vec::new(),
    };
    let list = || {
        Response::new(Body::from(
            json!({"metadata": {"resourceVersion": "10"}, "items": []})
                .to_string()
                .into_bytes(),
        ))
    };
    let forbidden = || {
        Response::builder()
            .status(403)
            .body(Body::from(
                json!({
                    "apiVersion": "v1", "kind": "Status", "status": "Failure",
                    "reason": "Forbidden", "code": 403,
                    "message": "pods is forbidden: cannot watch resource \"pods\"",
                })
                .to_string()
                .into_bytes(),
            ))
            .unwrap()
    };

    let mock = async {
        for _ in 0..2 {
            let (request, reply) = requests.next_request().await.unwrap();
            assert!(
                !request.uri().query().unwrap().contains("watch=true"),
                "a denied watch was retried without a relist"
            );
            reply.send_response(list());
            let (watch, reply) = requests.next_request().await.unwrap();
            assert!(watch.uri().query().unwrap().contains("watch=true"));
            reply.send_response(forbidden());
        }
    };
    tokio::select! {
        () = logs.watch_pods(&pods, "app=api") => panic!("the log stream ended"),
        () = mock => {},
    }
}

type MockBody = http_body_util::combinators::UnsyncBoxBody<bytes::Bytes, std::convert::Infallible>;

fn aggregate_logs(client: Client) -> AggregateLogs {
    AggregateLogs {
        client,
        namespace: "default".into(),
        options: LogStreamOptions {
            batch_lines: false,
            tail_lines: Some(10),
            since_seconds: None,
        },
        channel: Channel::new(|_| Ok(())),
        stream_id: "logs".into(),
        streams: SourceStreams::default(),
        pods: PodSources::default(),
        selected: Vec::new(),
    }
}

/// Serves the pods from `list` (called with the list's index), answers every log GET with a
/// waiting-container 400 and records its pod, and either holds one watch open for `watch`
/// events or denies every watch with a 403.
fn serve_pods(
    mut requests: tower_test::mock::Handle<Request<Body>, Response<MockBody>>,
    mut list: impl FnMut(usize) -> Vec<Pod> + Send + 'static,
    mut watch: Option<tokio::sync::mpsc::UnboundedReceiver<bytes::Bytes>>,
) -> (
    tokio::task::JoinHandle<()>,
    std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) {
    use bytes::Bytes;
    use http_body::Frame;
    use http_body_util::{BodyExt, Full, StreamBody};
    use std::convert::Infallible;

    let full = |status: u16, body: serde_json::Value| {
        Response::builder()
            .status(status)
            .body(Full::new(Bytes::from(body.to_string())).boxed_unsync())
            .unwrap()
    };
    let failure = |code: u16, reason: &str, message: &str| {
        json!({
            "apiVersion": "v1", "kind": "Status", "status": "Failure",
            "reason": reason, "code": code, "message": message,
        })
    };
    let logged = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = std::sync::Arc::clone(&logged);
    let mut lists = 0;
    let server = tokio::spawn(async move {
        while let Some((request, reply)) = requests.next_request().await {
            let path = request.uri().path().to_owned();
            let query = request.uri().query().unwrap_or_default().to_owned();
            if let Some(pod) = path.strip_suffix("/log") {
                let pod = pod.rsplit('/').next().unwrap_or_default().to_owned();
                recorded.lock().unwrap().push(pod);
                reply.send_response(full(
                    400,
                    failure(400, "BadRequest", "container is waiting to start"),
                ));
            } else if query.contains("watch=true") {
                let Some(receiver) = watch.take() else {
                    reply.send_response(full(
                        403,
                        failure(403, "Forbidden", "cannot watch resource \"pods\""),
                    ));
                    continue;
                };
                let frames = futures_util::stream::unfold(receiver, |mut receiver| async {
                    let bytes = receiver.recv().await?;
                    Some((Ok::<_, Infallible>(Frame::data(bytes)), receiver))
                });
                reply.send_response(Response::new(StreamBody::new(frames).boxed_unsync()));
            } else {
                let items = list(lists);
                lists += 1;
                reply.send_response(full(
                    200,
                    json!({"metadata": {"resourceVersion": "10"}, "items": items}),
                ));
            }
        }
    });
    (server, logged)
}

// Failure mode: when a retried source finishes, reconciling every source marks the other
// still-cooling source for retry, so two sources re-arm each other once a second forever
// after one pod event. Each source must retry once and then wait for the next pod event.
#[tokio::test(start_paused = true)]
async fn a_retried_source_does_not_re_arm_the_other_sources() {
    let (service, requests) = tower_test::mock::pair::<Request<Body>, Response<MockBody>>();
    let client = Client::new(service, "default");
    let pods = Api::namespaced(client.clone(), "default");
    let mut logs = aggregate_logs(client);
    let mut waiting = pod("api-a", &["app", "sidecar"]);
    waiting.metadata.resource_version = Some("10".into());
    let listed = waiting.clone();
    let (events, receiver) = tokio::sync::mpsc::unbounded_channel::<bytes::Bytes>();
    let (server, logged) = serve_pods(requests, move |_| vec![listed.clone()], Some(receiver));
    let log_requests = || logged.lock().unwrap().len();

    let check = async {
        tokio::time::sleep(Duration::from_secs(30)).await;
        assert_eq!(log_requests(), 2, "both containers attach once");
        let mut modify = |version: &str| {
            waiting.metadata.resource_version = Some(version.into());
            events
                .send(bytes::Bytes::from(format!(
                    "{}\n",
                    json!({"type": "MODIFIED", "object": waiting})
                )))
                .unwrap();
        };
        modify("11");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(log_requests(), 4, "a pod event retries both finished sources");
        // Both sources are cooling down, so this event asks each to retry once it ends.
        modify("12");
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(log_requests(), 6, "each source retries once after its cooldown");
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert_eq!(
            log_requests(),
            6,
            "finished sources kept reconnecting without a pod event"
        );
    };
    tokio::select! {
        () = logs.watch_pods(&pods, "app=api") => panic!("the log stream ended"),
        () = check => {},
    }
    server.abort();
}

// Failure mode: with list-only RBAC the pod watch returns 403 before it streams, so the loop
// only relists and never joins finished tasks. A source whose log ended keeps its handle, so
// relists never reconnect it, and the tasks of departed pods pile up in the JoinSet.
#[tokio::test(start_paused = true)]
async fn relists_under_a_denied_watch_reap_finished_sources() {
    let (service, requests) = tower_test::mock::pair::<Request<Body>, Response<MockBody>>();
    let client = Client::new(service, "default");
    let pods = Api::namespaced(client.clone(), "default");
    let mut logs = aggregate_logs(client);
    // `api-a` stays; every relist replaces the other pod with a new one.
    let (server, logged) = serve_pods(
        requests,
        |index| vec![pod("api-a", &["app"]), pod(&format!("api-churn-{index}"), &["app"])],
        None,
    );

    tokio::select! {
        () = logs.watch_pods(&pods, "app=api") => panic!("the log stream ended"),
        () = tokio::time::sleep(Duration::from_secs(30)) => {},
    }
    server.abort();
    let stable = logged
        .lock()
        .unwrap()
        .iter()
        .filter(|pod| *pod == "api-a")
        .count();
    assert!(stable > 1, "a relist never reconnected the finished api-a source");
    // At most the two selected sources' tasks and the departed pod's task that the next
    // wait would join.
    assert!(
        logs.streams.tasks.len() <= logs.selected.len() + 1,
        "{} tasks are held for {} sources",
        logs.streams.tasks.len(),
        logs.selected.len()
    );
}
