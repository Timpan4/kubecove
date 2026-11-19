use super::*;
use crate::backend::Read;
use kubecove_lib::models::AppError;
use std::task::{Context, Poll, Waker};

fn commands(recorder: &mut Recorder) -> Vec<String> {
    recorder
        .json_lines()
        .lines()
        .flat_map(|line| {
            let entry: Value = serde_json::from_str(line).expect("JSON line");
            entry["commands"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|command| command["cmd"].as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn completion_evicted_before_the_next_flush_stays_with_its_first_entry() {
    let timings = Timings::default();
    timings.set_enabled(true);
    let mut recorder = Recorder::default();
    recorder.start(&timings);
    recorder.begin(
        Kind::Click,
        "origin".into(),
        None,
        Instant::now(),
        Some(&timings),
    );
    let mut read = timings.measure("list_resource_topology", Box::pin(async { Ok(()) }));
    recorder.begin(
        Kind::Key,
        "next".into(),
        None,
        Instant::now(),
        Some(&timings),
    );
    assert!(matches!(
        read.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Ok(()))
    ));
    assert!(
        timings
            .snapshot()
            .iter()
            .any(|timing| timing.name == "list_resource_topology")
    );
    for _ in 0..crate::diagnostics::MAX_EVENTS {
        timings.record_presentation("resources.present", Instant::now());
    }
    let trace = timings.snapshot();
    assert_eq!(trace.len(), crate::diagnostics::MAX_EVENTS);
    assert!(
        trace
            .iter()
            .all(|timing| timing.name == "resources.present")
    );
    recorder.stop(Some(&timings));

    let report = recorder.json_lines();
    let entries: Vec<Value> = report
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(entries[0]["target"], "origin");
    assert_eq!(entries[0]["commands"][0]["cmd"], "list_resource_topology");
    assert_eq!(entries[0]["commands"][0]["ok"], true);
    assert_eq!(entries[1]["commands"], json!([]));
}

#[test]
fn clear_diagnostics_during_a_command_clears_latency_but_keeps_its_recording() {
    let timings = Timings::default();
    timings.set_enabled(true);
    let mut recorder = Recorder::default();
    recorder.start(&timings);
    recorder.begin(
        Kind::Click,
        "origin".into(),
        None,
        Instant::now(),
        Some(&timings),
    );
    let (finish, pending) = tokio::sync::oneshot::channel();
    let mut read = timings.measure(
        "list_resource_topology",
        Box::pin(async move {
            pending.await.expect("test sends the completion");
            Err::<(), _>(AppError::new(
                "private error",
                kubecove_lib::models::AppErrorKind::Internal,
            ))
        }),
    );
    let mut context = Context::from_waker(Waker::noop());
    assert!(read.as_mut().poll(&mut context).is_pending());
    recorder.begin(
        Kind::Key,
        "clear".into(),
        None,
        Instant::now(),
        Some(&timings),
    );
    timings.clear();
    finish.send(()).unwrap();
    assert!(matches!(
        read.as_mut().poll(&mut context),
        Poll::Ready(Err(_))
    ));
    assert!(timings.snapshot().is_empty());
    recorder.stop(Some(&timings));

    let report = recorder.json_lines();
    let entries: Vec<Value> = report
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(entries[0]["target"], "origin");
    assert_eq!(entries[0]["commands"][0]["cmd"], "list_resource_topology");
    assert_eq!(entries[0]["commands"][0]["ok"], false);
    assert_eq!(entries[1]["commands"], json!([]));
    assert!(!report.contains("private error"));
}

#[test]
fn reapplying_enabled_diagnostics_keeps_the_pending_recording() {
    let timings = Timings::default();
    timings.set_enabled(true);
    let mut recorder = Recorder::default();
    recorder.start(&timings);
    recorder.begin(
        Kind::Click,
        "origin".into(),
        None,
        Instant::now(),
        Some(&timings),
    );
    let (finish, pending) = tokio::sync::oneshot::channel();
    let mut read = timings.measure(
        "list_resource_topology",
        Box::pin(async move {
            pending.await.expect("test sends the completion");
            Ok(())
        }),
    );
    let mut context = Context::from_waker(Waker::noop());
    assert!(read.as_mut().poll(&mut context).is_pending());
    timings.set_enabled(true);
    finish.send(()).unwrap();
    assert!(matches!(
        read.as_mut().poll(&mut context),
        Poll::Ready(Ok(()))
    ));
    assert!(timings.snapshot().is_empty());
    recorder.stop(Some(&timings));
    assert_eq!(commands(&mut recorder), ["list_resource_topology"]);
}

#[test]
fn a_backdated_navigation_keeps_commands_with_the_interaction_that_started_them() {
    let timings = Timings::default();
    timings.set_enabled(true);
    let mut recorder = Recorder::default();
    recorder.start(&timings);
    recorder.begin(
        Kind::Click,
        "origin".into(),
        None,
        Instant::now(),
        Some(&timings),
    );
    let requested_at = Instant::now();
    let mut earlier = timings.measure("completed_before_navigation", Box::pin(async { Ok(()) }));
    let mut later = timings.measure("completed_after_navigation", Box::pin(async { Ok(()) }));
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(
        earlier.as_mut().poll(&mut context),
        Poll::Ready(Ok(()))
    ));
    recorder.begin(
        Kind::Navigate,
        "navigation".into(),
        None,
        requested_at,
        Some(&timings),
    );
    assert!(matches!(
        later.as_mut().poll(&mut context),
        Poll::Ready(Ok(()))
    ));
    recorder.stop(Some(&timings));

    let report = recorder.json_lines();
    let entries: Vec<Value> = report
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let origin: Vec<_> = entries[0]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["cmd"].as_str().unwrap())
        .collect();
    assert_eq!(
        origin,
        ["completed_before_navigation", "completed_after_navigation"]
    );
    assert_eq!(entries[1]["commands"], json!([]));
}

// Browser mocks cannot hold a page read while stopping recording in Settings.
#[test]
fn copies_keep_an_earlier_completion_and_add_a_late_one_once() {
    let timings = Timings::default();
    timings.set_enabled(true);
    let mut recorder = Recorder::default();
    recorder.start(&timings);
    recorder.begin(
        Kind::Click,
        "button".into(),
        None,
        Instant::now(),
        Some(&timings),
    );
    let mut context = Context::from_waker(Waker::noop());
    let mut earlier = timings.measure("get_resource_details", Box::pin(async { Ok(()) }));
    assert!(matches!(
        earlier.as_mut().poll(&mut context),
        Poll::Ready(Ok(()))
    ));
    let (finish, pending) = tokio::sync::oneshot::channel();
    let read: Read<()> = Box::pin(async move {
        pending.await.expect("test sends the completion");
        Ok(())
    });
    let mut later = timings.measure("list_resource_topology", read);
    assert!(later.as_mut().poll(&mut context).is_pending());
    recorder.stop(Some(&timings));

    assert_eq!(commands(&mut recorder), ["get_resource_details"]);
    finish.send(()).unwrap();
    assert!(matches!(
        later.as_mut().poll(&mut context),
        Poll::Ready(Ok(()))
    ));
    timings.clear();
    let expected = ["get_resource_details", "list_resource_topology"];
    assert_eq!(commands(&mut recorder), expected);
    assert_eq!(commands(&mut recorder), expected);
}
