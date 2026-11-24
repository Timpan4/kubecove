use super::*;
use http::{Request, Response};
use k8s_openapi::{
    api::core::v1::{Container, PodSpec},
    apimachinery::pkg::apis::meta::v1::Time,
};
use kube::{client::Body, core::ObjectMeta};
use serde_json::json;

#[path = "aggregate_logs/source_tests.rs"]
mod source_tests;

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
fn log_sources_skip_deleting_pods_and_cap_results() {
    let mut deleting = pod("api-old", &["app"]);
    deleting.metadata.deletion_timestamp =
        Some(Time(k8s_openapi::jiff::Timestamp::from_second(0).unwrap()));
    let pods = std::iter::once(deleting)
        .chain((0..60).map(|index| pod(&format!("api-{index:02}"), &["app"])))
        .collect::<Vec<_>>();

    let index = PodSources::from_pods(&pods);
    let sources = index.selected();

    assert_eq!(index.total(), 60);
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

// A watch denial switches to list-only discovery for the lifetime of the stream.
#[tokio::test(start_paused = true)]
async fn a_denied_watch_polls_with_one_warning_and_steady_status() {
    for in_watch_event in [false, true] {
        let (service, mut requests) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
        let client = Client::new(service, "default");
        let pods = Api::namespaced(client.clone(), "default");
        let mut logs = aggregate_logs(client);
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&received);
        logs.options.batch_lines = in_watch_event;
        logs.channel = Channel::new(move |message| {
            sink.lock().unwrap().push(message);
            Ok(())
        });

        let list = || {
            Response::new(Body::from(
                json!({"metadata": {"resourceVersion": "10"}, "items": []})
                    .to_string()
                    .into_bytes(),
            ))
        };
        let mock = async {
            let (request, reply) = requests.next_request().await.unwrap();
            assert!(!request.uri().query().unwrap().contains("watch=true"));
            reply.send_response(list());

            let (watch, reply) = requests.next_request().await.unwrap();
            assert!(watch.uri().query().unwrap().contains("watch=true"));
            let status = json!({
                "apiVersion": "v1", "kind": "Status", "status": "Failure",
                "reason": "Forbidden", "code": 403, "message": "cannot watch pods",
            });
            let forbidden = if in_watch_event {
                Response::new(Body::from(
                    format!("{}\n", json!({"type": "ERROR", "object": status})).into_bytes(),
                ))
            } else {
                Response::builder()
                    .status(403)
                    .body(Body::from(status.to_string().into_bytes()))
                    .unwrap()
            };
            reply.send_response(forbidden);

            // Observe multiple requests after the denial, including two completed polls.
            for _ in 0..3 {
                let (request, reply) = requests.next_request().await.unwrap();
                assert!(
                    !request.uri().query().unwrap().contains("watch=true"),
                    "pod discovery retried the forbidden watch"
                );
                reply.send_response(list());
            }
        };
        tokio::select! {
            () = logs.watch_pods(&pods, "app=api") => panic!("the log stream ended"),
            () = mock => {},
        }

        let received = received.lock().unwrap();
        let mut statuses = Vec::new();
        let mut warnings = Vec::new();
        for message in received.iter() {
            match message {
                StreamMessage::Status {
                    status, message, ..
                } => statuses.push((*status, message)),
                StreamMessage::LogLine {
                    line, source: None, ..
                } if !in_watch_event => {
                    warnings.push(line);
                }
                StreamMessage::LogLines {
                    lines,
                    source: None,
                    ..
                } if in_watch_event => {
                    warnings.extend(lines);
                }
                _ => panic!("unexpected log message: {message:?}"),
            }
        }
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("permission denied"));
        assert!(warnings[0].contains("Polling"));
        assert_eq!(statuses.len(), 2);
        assert!(
            statuses
                .iter()
                .all(|(status, _)| *status == StreamStatus::Connected)
        );
        assert!(statuses[1].1.contains("Polling"));
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
        |index| {
            vec![
                pod("api-a", &["app"]),
                pod(&format!("api-churn-{index}"), &["app"]),
            ]
        },
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
    assert!(
        stable > 1,
        "a relist never reconnected the finished api-a source"
    );
    // At most the two selected sources' tasks and the departed pod's task that the next
    // wait would join.
    assert!(
        logs.streams.tasks.len() <= logs.selected.len() + 1,
        "{} tasks are held for {} sources",
        logs.streams.tasks.len(),
        logs.selected.len()
    );
}
