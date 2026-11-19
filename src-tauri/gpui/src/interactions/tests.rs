use super::*;
use crate::backend::Read;
use kubecove_lib::models::AppError;
use std::task::{Context, Poll, Waker};

fn commands(recorder: &Recorder) -> Vec<String> {
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
    let mut read = timings.measure(
        "list_resource_topology",
        Box::pin(async {
            Err::<(), _>(AppError::new(
                "private error",
                kubecove_lib::models::AppErrorKind::Internal,
            ))
        }),
    );
    recorder.begin(
        Kind::Key,
        "clear".into(),
        None,
        Instant::now(),
        Some(&timings),
    );
    timings.clear();
    assert!(matches!(
        read.as_mut().poll(&mut Context::from_waker(Waker::noop())),
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

// Failure mode (#662): a command still running when recording stops shows up
// in one copy, then Clear diagnostics removes it from the next, because no
// flush follows the stop. Browser mocks cannot reach this: the only way to
// stop recording is to open Settings, which cancels every held page read.
#[test]
fn a_command_finishing_after_stop_stays_after_the_trace_is_cleared() {
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
    let read: Read<()> = Box::pin(async { Ok::<(), AppError>(()) });
    let mut read = timings.measure("list_resource_topology", read);
    recorder.stop(Some(&timings));
    let finished = read.as_mut().poll(&mut Context::from_waker(Waker::noop()));
    assert!(matches!(finished, Poll::Ready(Ok(()))));

    assert_eq!(commands(&recorder), ["list_resource_topology"]);
    timings.clear();
    assert_eq!(commands(&recorder), ["list_resource_topology"]);
}
