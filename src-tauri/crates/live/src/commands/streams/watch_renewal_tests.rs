use super::*;
use crate::{commands::streams::StreamRegistry, test_support::stream_channel};
use http::{Request, Response};
use kube::client::Body;
use serde_json::{json, Value};
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
        if reconnecting {
            Err(())
        } else {
            Ok(())
        }
    });
    let key: WatchResourceKey =
        serde_json::from_value(json!({ "resourceKind": { "kind": "Pod" } })).unwrap();
    let registry = StreamRegistry::default();
    let (_, broadcaster, _) = registry.subscribe_resource("test", "source", "dev", &key, channel);
    run_resource_watch_with_client(
        "source".into(),
        "dev".into(),
        key,
        broadcaster,
        ClusterLiveStore::default(),
        || std::future::ready(Ok(client.clone())),
    )
    .await;

    let requests = requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        2,
        "normal expiry must reopen without announcing a disconnect"
    );
    assert!(requests[1].contains("resourceVersion=55"));
    let messages = messages.lock().unwrap();
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0]["status"], "connected");
    assert_eq!(messages[1]["status"], "connected");
    assert_eq!(messages[2]["type"], "error");
    assert!(messages[2]["message"]
        .as_str()
        .unwrap()
        .contains("watch unavailable"));
    assert_eq!(messages[3]["status"], "reconnecting");
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
            if reconnecting {
                Err(())
            } else {
                Ok(())
            }
        });
        let key: WatchResourceKey = serde_json::from_value(
            json!({ "resourceKind": { "kind": "Secret" }, "namespace": "cert-manager" }),
        )
        .unwrap();
        let registry = StreamRegistry::default();
        let (_, broadcaster, _) =
            registry.subscribe_resource("test", "source", "dev", &key, channel);
        run_resource_watch_with_client(
            "source".into(),
            "dev".into(),
            key.clone(),
            broadcaster,
            ClusterLiveStore::default(),
            || std::future::ready(Ok(client.clone())),
        )
        .await;
        assert_eq!(*requests.lock().unwrap(), 1);
        let messages = messages.lock().unwrap();
        assert_eq!(messages.last().unwrap()["status"], "forbidden");
        assert_eq!(messages.last().unwrap()["message"], "Secret in namespace cert-manager: watch access denied. Readable resources will refresh periodically.");
        assert!(messages
            .iter()
            .all(|message| message["status"] != "reconnecting"));
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
