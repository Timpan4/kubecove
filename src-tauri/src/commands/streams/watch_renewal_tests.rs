use super::*;
use crate::{commands::streams::StreamRegistry, models::StreamMessage};
use http::{Request, Response};
use kube::client::Body;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody};

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
                .status(403)
                .body(Body::from(
                    json!({
                        "kind": "Status", "apiVersion": "v1", "status": "Failure",
                        "reason": "Forbidden", "message": "watch denied", "code": 403
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
    let channel = Channel::<StreamMessage>::new(move |body| {
        let InvokeResponseBody::Json(body) = body else {
            panic!("expected JSON")
        };
        let message: Value = serde_json::from_str(&body).unwrap();
        let reconnecting = message["status"] == "reconnecting";
        observed_messages.lock().unwrap().push(message);
        // Close the consumer at the first actual disconnect, ending the watch task.
        if reconnecting {
            Err(std::io::Error::other("consumer closed").into())
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
        .contains("watch denied"));
    assert_eq!(messages[3]["status"], "reconnecting");
}
