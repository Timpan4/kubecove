use super::*;
use crate::{
    commands::{ClientIntent, FixedClusterAccess, streams::StreamRegistry},
    test_support::stream_channel,
};
use http::{Request, Response};
use kube::client::Body;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn normal_watch_expiry_renews_without_recovery_and_preserves_version() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed_requests = requests.clone();
    let service = tower::service_fn(move |request: Request<Body>| {
        let mut requests = observed_requests.lock().unwrap();
        requests.push(request.uri().to_string());
        let response = if requests.len() == 1 {
            Response::builder().body(Body::from(format!("{}\n", json!({
                "type": "BOOKMARK",
                "object": { "apiVersion": "v1", "kind": "Pod", "metadata": { "resourceVersion": "55" } }
            })).into_bytes())).unwrap()
        } else {
            Response::builder()
                .status(503)
                .body(Body::from(
                    json!({
                        "kind": "Status", "apiVersion": "v1", "status": "Failure",
                        "reason": "ServiceUnavailable", "message": "watch unavailable", "code": 503
                    })
                    .to_string()
                    .into_bytes(),
                ))
                .unwrap()
        };
        std::future::ready(Ok::<_, std::convert::Infallible>(response))
    });
    let client = Client::new(service, "default");
    let messages = Arc::new(Mutex::new(Vec::<Value>::new()));
    let observed_messages = messages.clone();
    let channel = stream_channel(move |message| {
        let reconnecting = message["status"] == "reconnecting";
        observed_messages.lock().unwrap().push(message);
        // Close the consumer at the first actual disconnect, ending the watch task.
        if reconnecting { Err(()) } else { Ok(()) }
    });
    let key: ResourceWatchStart =
        serde_json::from_value(json!({ "resourceKind": { "kind": "Pod" } })).unwrap();
    let registry = StreamRegistry::default();
    let (_, broadcaster, _) = registry.subscribe_resource("test", "source", "dev", &key, channel);
    let cluster = FixedClusterAccess::new(client);
    run_resource_watch(
        cluster.access(),
        "dev".into(),
        key,
        broadcaster,
        ClusterLiveStore::default(),
    )
    .await;
    // Each reconnect asks for a long-lived stream client again.
    assert_eq!(cluster.intents(), vec![ClientIntent::Stream; 2]);

    let requests = requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        2,
        "normal expiry must reopen without announcing a disconnect"
    );
    assert!(requests[1].contains("resourceVersion=55"));
    let messages = messages.lock().unwrap();
    // The refused renewal fails before it streams, so it is never reported as connected.
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["status"], "connected");
    assert_eq!(messages[1]["type"], "error");
    assert!(
        messages[1]["message"]
            .as_str()
            .unwrap()
            .contains("watch unavailable")
    );
    assert_eq!(messages[2]["status"], "reconnecting");
}

#[tokio::test(start_paused = true)]
async fn event_watch_resets_version_after_in_stream_internal_error() {
    for reply in [EventWatchError::Envelope, EventWatchError::BareStatus] {
        let (versions, states) = recover_event_watch_after_error(reply, 500).await;
        assert_eq!(versions, ["0", "55", "0"]);
        assert_eq!(
            states,
            [
                "connected",
                "reconnecting",
                "connected",
                "error",
                "reconnecting",
                "connected",
                "resourceEventsChanged"
            ]
        );
    }
}

#[tokio::test(start_paused = true)]
async fn event_watch_resumes_from_a_bookmark_received_before_an_in_stream_503() {
    let (versions, _) = recover_event_watch_after_error(EventWatchError::Envelope, 503).await;
    assert_eq!(versions, ["0", "55", "56"]);
}

#[tokio::test(start_paused = true)]
async fn event_watch_uses_the_right_version_after_an_http_error() {
    for (code, expected_version) in [(410, "0"), (500, "0"), (503, "55")] {
        let (versions, states) = recover_event_watch_after_error(EventWatchError::Http, code).await;
        assert_eq!(versions, ["0", "55", expected_version], "HTTP {code}");
        assert_eq!(
            states,
            [
                "connected",
                "reconnecting",
                "error",
                "reconnecting",
                "connected",
                "resourceEventsChanged"
            ],
            "HTTP {code}"
        );
    }
}

#[derive(Clone, Copy)]
enum EventWatchError {
    Envelope,
    BareStatus,
    Http,
}

async fn recover_event_watch_after_error(
    reply: EventWatchError,
    code: u16,
) -> (Vec<String>, Vec<String>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let service_requests = requests.clone();
    let service = tower::service_fn(move |request: Request<Body>| {
        let mut requests = service_requests.lock().unwrap();
        requests.push(request.uri().to_string());
        let bookmark = |version| {
            json!({
                "type": "BOOKMARK",
                "object": { "apiVersion": "v1", "kind": "Event", "metadata": { "resourceVersion": version } }
            })
        };
        let error = json!({
            "kind": "Status", "apiVersion": "v1", "status": "Failure", "code": code,
            "message": if code == 500 { "etcd event received with PrevKv=nil (type=DELETE)" } else { "Events watch rejected" }
        });
        let (code, body) = match (reply, requests.len()) {
            (_, 1) => (200, format!("{}\n", bookmark("55"))),
            (EventWatchError::Envelope, 2) if code == 503 => (
                200,
                format!(
                    "{}\n{}\n",
                    bookmark("56"),
                    json!({ "type": "ERROR", "object": error })
                ),
            ),
            (EventWatchError::Envelope, 2) => (
                200,
                format!("{}\n", json!({ "type": "ERROR", "object": error })),
            ),
            (EventWatchError::BareStatus, 2) => (200, format!("{error}\n")),
            (EventWatchError::Http, 2) => (code, error.to_string()),
            (_, 3) => (
                200,
                format!(
                    "{}\n",
                    json!({
                        "type": "ADDED",
                        "object": {
                            "apiVersion": "v1",
                            "kind": "Event",
                            "metadata": { "name": "app.started", "namespace": "default", "resourceVersion": "56" },
                            "involvedObject": { "kind": "Pod", "name": "app", "namespace": "default" }
                        }
                    })
                ),
            ),
            (_, attempt) => (
                503,
                json!({ "kind": "Status", "apiVersion": "v1", "status": "Failure",
                    "code": 503, "message": format!("Unexpected Events watch attempt {attempt}") })
                .to_string(),
            ),
        };
        std::future::ready(Ok::<_, std::convert::Infallible>(
            Response::builder()
                .status(code)
                .body(Body::from(body.into_bytes()))
                .unwrap(),
        ))
    });
    let messages = Arc::new(Mutex::new(Vec::<Value>::new()));
    let observed_messages = messages.clone();
    let observed_requests = requests.clone();
    let channel = stream_channel(move |message| {
        let delivered = message["type"] == "resourceEventsChanged";
        if delivered {
            assert_eq!(message["target"]["name"], "app");
            assert_eq!(message["action"], "added");
        }
        // Close on an unexpected retry so missing Event delivery fails the assertions.
        let unexpected_retry =
            message["type"] == "error" && observed_requests.lock().unwrap().len() > 3;
        observed_messages.lock().unwrap().push(message);
        if delivered || unexpected_retry {
            Err(())
        } else {
            Ok(())
        }
    });
    let registry = StreamRegistry::default();
    let (_, broadcaster, _) = registry.subscribe_event(
        "test",
        "source",
        "dev",
        "Pod",
        "app",
        Some("default"),
        channel,
    );
    run_event_watch(
        FixedClusterAccess::new(Client::new(service, "default")).access(),
        "dev".into(),
        "Pod".into(),
        "app".into(),
        Some("default".into()),
        broadcaster,
    )
    .await;

    let versions = requests
        .lock()
        .unwrap()
        .iter()
        .map(|uri| {
            uri.split(['?', '&'])
                .find_map(|parameter| parameter.strip_prefix("resourceVersion="))
                .unwrap()
                .to_owned()
        })
        .collect();
    let states = messages
        .lock()
        .unwrap()
        .iter()
        .map(|message| {
            message["status"]
                .as_str()
                .or_else(|| message["type"].as_str())
                .unwrap()
                .to_owned()
        })
        .collect();
    (versions, states)
}

#[tokio::test]
async fn forbidden_watch_stops_and_replays_permission_status_to_new_subscribers() {
    for in_stream in [false, true] {
        let requests = Arc::new(Mutex::new(0));
        let observed_requests = requests.clone();
        let service = tower::service_fn(move |_: Request<Body>| {
            *observed_requests.lock().unwrap() += 1;
            let status = json!({ "kind": "Status", "apiVersion": "v1", "status": "Failure", "reason": "Forbidden", "message": "secrets watch denied", "code": 403 });
            let body = if in_stream {
                format!("{}\n", json!({ "type": "ERROR", "object": status }))
            } else {
                status.to_string()
            };
            std::future::ready(Ok::<_, std::convert::Infallible>(
                Response::builder()
                    .status(if in_stream { 200 } else { 403 })
                    .body(Body::from(body.into_bytes()))
                    .unwrap(),
            ))
        });
        let client = Client::new(service, "default");
        let messages = Arc::new(Mutex::new(Vec::<Value>::new()));
        let observed_messages = messages.clone();
        let channel = stream_channel(move |message| {
            let reconnecting = message["status"] == "reconnecting";
            observed_messages.lock().unwrap().push(message);
            // Old behavior exits through this callback so the regression cannot hang.
            if reconnecting { Err(()) } else { Ok(()) }
        });
        let key: ResourceWatchStart = serde_json::from_value(
            json!({ "resourceKind": { "kind": "Secret" }, "namespace": "cert-manager" }),
        )
        .unwrap();
        let registry = StreamRegistry::default();
        let (_, broadcaster, _) =
            registry.subscribe_resource("test", "source", "dev", &key, channel);
        run_resource_watch(
            FixedClusterAccess::new(client.clone()).access(),
            "dev".into(),
            key.clone(),
            broadcaster,
            ClusterLiveStore::default(),
        )
        .await;
        assert_eq!(*requests.lock().unwrap(), 1);
        let messages = messages.lock().unwrap();
        assert_eq!(messages.last().unwrap()["status"], "forbidden");
        assert_eq!(
            messages.last().unwrap()["message"],
            "Secret in namespace cert-manager: watch access denied. Readable resources will refresh periodically."
        );
        assert!(
            messages
                .iter()
                .all(|message| message["status"] != "reconnecting")
        );
        drop(messages);
        let replay = Arc::new(Mutex::new(Vec::<Value>::new()));
        let observed_replay = replay.clone();
        let channel = stream_channel(move |message| {
            observed_replay.lock().unwrap().push(message);
            Ok(())
        });
        let (_, _, started) = registry.subscribe_resource("late", "source", "dev", &key, channel);
        assert!(!started);
        assert_eq!(replay.lock().unwrap()[0]["status"], "forbidden");
        registry.stop("late");
        registry.stop("test");
    }
}
