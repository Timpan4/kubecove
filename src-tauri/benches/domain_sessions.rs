use codspeed_divan_compat::{black_box, Bencher};
use kubecove_lib::commands::bench_support::domain_catalog::domain_sessions;
use kubecove_lib::models::AppErrorKind;

mod service_ready_target_resolution_named {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn service_ready_target_resolution_named(bencher: Bencher) {
        let (pods, named_port, numeric_port) = domain_sessions::target_inputs();
        let named = domain_sessions::resolve_service_target(pods.clone(), named_port.clone())
            .expect("second eligible Pod exposes the named port");
        assert_eq!(named, ("api-b".to_string(), 8080));
        let numeric = domain_sessions::resolve_service_target(pods.clone(), numeric_port.clone())
            .expect("numeric target resolves on first ready Pod");
        assert_eq!(numeric, ("api-a".to_string(), 9090));
        let unavailable = domain_sessions::resolve_service_target(Vec::new(), named_port.clone())
            .expect_err("empty Pod list cannot satisfy a named target");
        assert_eq!(unavailable.kind, AppErrorKind::LiveSessionTargetUnavailable);
        assert_eq!(
            unavailable.message,
            "no ready Pods matched this Service selector"
        );
        let api_without_named_port = pods
            .iter()
            .filter(|pod| pod.metadata.name.as_deref() == Some("api-a"))
            .cloned()
            .collect();
        let missing_port =
            domain_sessions::resolve_service_target(api_without_named_port, named_port.clone())
                .expect_err("ready Pod without the named port cannot resolve a target");
        assert_eq!(missing_port.kind, AppErrorKind::Session);
        assert_eq!(
            missing_port.message,
            "Service targetPort 'http' was not found on the resolved Pod"
        );

        bencher
            .with_inputs(|| (pods.clone(), named_port.clone()))
            .bench_values(|(pods, port)| {
                black_box(domain_sessions::resolve_service_target(pods, port))
            });
    }
}

mod service_ready_target_resolution_numeric {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn service_ready_target_resolution_numeric(bencher: Bencher) {
        let (pods, _, numeric_port) = domain_sessions::target_inputs();
        let expected = domain_sessions::resolve_service_target(pods.clone(), numeric_port.clone())
            .expect("numeric target resolves on first ready Pod");
        assert_eq!(expected, ("api-a".to_string(), 9090));

        bencher
            .with_inputs(|| (pods.clone(), numeric_port.clone()))
            .bench_values(|(pods, port)| {
                black_box(domain_sessions::resolve_service_target(pods, port))
            });
    }
}

mod port_forward_session_summary_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn port_forward_session_summary_json_encoding(bencher: Bencher) {
        let summary = domain_sessions::port_forward_session_summary("api-b".to_string(), 8080);
        let json = domain_sessions::encode(&summary);
        assert!(json.contains("resolvedPodPort"));
        assert!(json.contains("2026-10-11T00:00:00Z"));
        bencher
            .with_inputs(|| summary.clone())
            .bench_values(|summary| black_box(domain_sessions::encode(&summary)));
    }
}

mod pod_exec_session_summary_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn pod_exec_session_summary_json_encoding(bencher: Bencher) {
        let summary = domain_sessions::pod_exec_session_summary();
        let json = domain_sessions::encode(&summary);
        assert!(json.contains("terminalCols"));
        assert!(json.contains("printf benchmark"));
        bencher
            .with_inputs(|| summary.clone())
            .bench_values(|summary| black_box(domain_sessions::encode(&summary)));
    }
}
