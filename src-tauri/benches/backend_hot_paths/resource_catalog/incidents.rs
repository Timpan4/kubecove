use super::*;

// R40: raw Events/Pods through warning shaping, restart joins, evidence, and incident ordering.
mod incident_cockpit_post_fetch_500_events {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn incident_cockpit_post_fetch_500_events(bencher: codspeed_divan_compat::Bencher) {
        // Events cover the first 167 Pods. Derive remaining membership from
        // prepared summary evidence, independently of the incident output.
        let expected_keys = INCIDENT_INPUT
            .resources
            .iter()
            .enumerate()
            .filter(|(index, resource)| {
                *index < 167
                    || resource.restarts.unwrap_or_default() > 0
                    || matches!(
                        resource.health_assessment.state,
                        kubecove_lib::models::HealthAssessmentState::NeedsAttention
                            | kubecove_lib::models::HealthAssessmentState::Degraded
                    )
            })
            .map(|(_, resource)| (&resource.namespace, &resource.name))
            .collect::<std::collections::BTreeSet<_>>();
        let actual_keys = INCIDENT_OUTPUT
            .items
            .iter()
            .map(|item| (&item.resource.namespace, &item.resource.name))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(actual_keys, expected_keys);
        assert_eq!(INCIDENT_OUTPUT.items.len(), expected_keys.len());
        assert_eq!(INCIDENT_OUTPUT.requested_scope[0].namespace, None);
        let mut invalid_timestamp =
            serde_json::to_value(&INCIDENT_INPUT.events[0]).expect("Event fixture serializes");
        invalid_timestamp["eventTime"] = serde_json::json!("not-a-timestamp");
        assert!(
            serde_json::from_value::<k8s_openapi::api::core::v1::Event>(invalid_timestamp).is_err()
        );
        let missing_timestamp = INCIDENT_OUTPUT
            .items
            .iter()
            .find(|item| item.resource.name == "ledger-0")
            .expect("warning incident with missing time");
        assert!(missing_timestamp
            .signals
            .iter()
            .any(|signal| signal.kind == "event" && signal.last_seen_at.is_none()));
        for (namespace, expected_state) in [
            (
                "collision-active",
                kubecove_lib::models::IncidentSignalState::Active,
            ),
            (
                "collision-historical",
                kubecove_lib::models::IncidentSignalState::Historical,
            ),
        ] {
            let item = INCIDENT_OUTPUT
                .items
                .iter()
                .find(|item| {
                    item.resource.name == "shared-name"
                        && item.resource.namespace.as_deref() == Some(namespace)
                })
                .expect("namespace-qualified incident");
            assert!(item
                .signals
                .iter()
                .any(|signal| signal.kind == "restart" && signal.state == expected_state));
        }
        assert_eq!(
            INCIDENT_OUTPUT
                .items
                .iter()
                .map(|item| item.warning_event_count)
                .sum::<usize>(),
            500
        );
        let order_keys = INCIDENT_OUTPUT
            .items
            .iter()
            .map(|item| {
                use kubecove_lib::models::{IncidentSeverity, IncidentSignalState};
                let state = match item.state {
                    IncidentSignalState::Active => 0,
                    IncidentSignalState::Resolved => 1,
                    IncidentSignalState::Historical => 2,
                };
                let severity = match item.severity {
                    IncidentSeverity::Degraded => 0,
                    IncidentSeverity::Attention => 1,
                    IncidentSeverity::Restarted => 2,
                    IncidentSeverity::Warning => 3,
                };
                assert!(
                    item.signals
                        .iter()
                        .filter(|signal| signal.kind == "event")
                        .count()
                        <= 3
                );
                (
                    state,
                    severity,
                    std::cmp::Reverse(item.latest_signal_at.as_deref()),
                    &item.resource.namespace,
                    &item.resource.kind,
                    &item.resource.name,
                )
            })
            .collect::<Vec<_>>();
        assert!(order_keys.windows(2).all(|pair| pair[0] <= pair[1]));
        bencher
            .with_inputs(|| INCIDENT_INPUT.clone())
            .bench_values(|fixture| black_box(prepare_incident_summary(fixture)));
    }
}

// R41: server-filtered selected-resource Events through UID filtering and timestamp projection.
mod selected_resource_events_500 {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn selected_resource_events_500(bencher: codspeed_divan_compat::Bencher) {
        assert_eq!(EVENT_OUTPUT.len(), 375);
        assert!(EVENT_OUTPUT
            .iter()
            .all(|event| event.last_seen_at.is_some()));
        assert!(EVENT_OUTPUT
            .iter()
            .any(|event| event.source == "legacy-kubelet"));
        let precedence = EVENT_OUTPUT
            .iter()
            .find(|event| {
                event.message == "deterministic warning event 0 with a retained diagnostic message"
            })
            .expect("precedence event retained");
        assert_eq!(
            precedence.last_seen_at.as_deref(),
            Some("2026-09-30T12:00:00+00:00")
        );
        assert_eq!(precedence.source, "preferred-controller");
        for pair in EVENT_OUTPUT.windows(2) {
            assert!(pair[0].last_seen_at >= pair[1].last_seen_at);
            if pair[0].last_seen_at == pair[1].last_seen_at {
                let first = EVENT_INPUT
                    .events
                    .iter()
                    .position(|event| event.message.as_deref() == Some(&pair[0].message))
                    .expect("first input event");
                let second = EVENT_INPUT
                    .events
                    .iter()
                    .position(|event| event.message.as_deref() == Some(&pair[1].message))
                    .expect("second input event");
                assert!(
                    first > second,
                    "equal timestamps follow stable sort then reverse"
                );
            }
        }
        bencher
            .with_inputs(|| EVENT_INPUT.clone())
            .bench_values(|fixture| black_box(prepare_resource_events(fixture)));
    }
}
