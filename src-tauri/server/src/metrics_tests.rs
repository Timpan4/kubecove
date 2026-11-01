use super::*;
use crate::observe::{AuthMode, CloseReason};
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use kubecove_lib::models::{AppError, NamespaceSummary};
use tower::ServiceExt;

fn sessions() -> Arc<Sessions> {
    Arc::new(Sessions::new(Duration::from_mins(5), AuthMode::Token))
}

/// The value of the sample `series`, such as `name{label="value"}`.
fn sample(text: &str, series: &str) -> f64 {
    text.lines()
        .find_map(|line| line.strip_prefix(series)?.strip_prefix(' '))
        .unwrap_or_else(|| panic!("{series} is missing:\n{text}"))
        .parse()
        .unwrap()
}

async fn read_namespaces(store: &ClusterLiveStore) {
    store
        .namespaces("source".into(), "context".into(), || async {
            Ok::<_, AppError>(vec![NamespaceSummary {
                name: "namespace".into(),
                age: "1m".into(),
                created_at: None,
            }])
        })
        .await
        .unwrap();
}

// Failure modes: the listener exposes an API, asset or WebSocket route; a
// required metric family is missing; the output is not OpenMetrics text.
#[tokio::test]
async fn metrics_listener_serves_only_metrics() {
    let app = router(sessions());
    let response = app
        .clone()
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("application/openmetrics-text"));
    let text = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .into(),
    )
    .unwrap();
    assert!(text.ends_with("# EOF\n"));
    // Labelled families appear once they have a sample; these always do.
    for family in [
        "kubecove_server_sessions_open",
        "kubecove_server_sessions_expired",
        "process_resident_memory_bytes",
        "process_cpu_seconds",
    ] {
        assert!(
            text.contains(&format!("# TYPE {family} ")),
            "{family} is missing"
        );
    }
    assert!(sample(&text, "process_resident_memory_bytes") > 0.0);
    for (method, path) in [
        (Method::GET, "/"),
        (Method::GET, "/index.html"),
        (Method::GET, "/api/v1/auth"),
        (Method::POST, "/api/v1/session"),
        (Method::POST, "/api/v1/invoke/get_backend_diagnostics"),
        (Method::GET, "/api/v1/channels"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    let response = app
        .oneshot(Request::post("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

// Failure modes: Settings -> Diagnostics "Clear" or a session ending lowers
// an exported counter, which Prometheus reads as a restart; a session's
// counters are counted twice; cache gauges keep an ended session's entries;
// Clear stops resetting the diagnostics table.
#[tokio::test]
async fn cache_counters_never_decrease_across_clear_and_session_end() {
    let metrics = Metrics::default();
    let sessions = sessions();
    let hits = "kubecove_server_cache_hits_total{cache=\"namespaces\"}";
    let misses = "kubecove_server_cache_misses_total{cache=\"namespaces\"}";
    let ready = "kubecove_server_cache_ready_entries{cache=\"namespaces\"}";
    let open = "kubecove_server_sessions_open";

    let first = sessions.authenticate_as(None, None).await.unwrap();
    let owner = sessions.owner(&first.resume_token).await.unwrap();
    read_namespaces(&owner.backend.live_store).await;
    read_namespaces(&owner.backend.live_store).await;
    let text = metrics.render(&sessions).await;
    assert_eq!(
        (
            sample(&text, hits),
            sample(&text, misses),
            sample(&text, ready),
            sample(&text, open)
        ),
        (1.0, 1.0, 1.0, 1.0)
    );

    owner.backend.live_store.clear_diagnostic_counters();
    let table = owner.backend.live_store.diagnostics();
    assert!(table
        .iter()
        .all(|cache| cache.hits == 0 && cache.misses == 0));
    read_namespaces(&owner.backend.live_store).await;
    let text = metrics.render(&sessions).await;
    assert_eq!((sample(&text, hits), sample(&text, misses)), (2.0, 1.0));

    sessions
        .close(&first.resume_token, CloseReason::Client)
        .await
        .unwrap();
    let text = metrics.render(&sessions).await;
    assert_eq!(
        (
            sample(&text, hits),
            sample(&text, misses),
            sample(&text, ready),
            sample(&text, open)
        ),
        (2.0, 1.0, 0.0, 0.0)
    );

    let second = sessions.authenticate_as(None, None).await.unwrap();
    let owner = sessions.owner(&second.resume_token).await.unwrap();
    read_namespaces(&owner.backend.live_store).await;
    let text = metrics.render(&sessions).await;
    assert_eq!(
        (
            sample(&text, hits),
            sample(&text, misses),
            sample(&text, open)
        ),
        (2.0, 2.0, 1.0)
    );
}
