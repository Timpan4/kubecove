use super::*;

fn workload<'a>(
    summary: &'a ResourceMetricsSummary,
    kind: &str,
    namespace: &str,
    name: &str,
) -> &'a kubecove_lib::models::ResourceMetricSummary {
    summary
        .workloads
        .iter()
        .find(|metric| {
            metric.kind == kind
                && metric.namespace.as_deref() == Some(namespace)
                && metric.name == name
        })
        .expect("expected workload metric")
}

fn pod_metric<'a>(
    summary: &'a ResourceMetricsSummary,
    namespace: &str,
    name: &str,
) -> &'a kubecove_lib::models::ResourceMetricSummary {
    summary
        .pods
        .iter()
        .find(|metric| metric.namespace.as_deref() == Some(namespace) && metric.name == name)
        .expect("expected Pod metric")
}

// R39, folded R38: metrics.k8s.io normalization, joins, and workload rollup.
mod resource_metrics_normalization_and_rollup_10000 {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn resource_metrics_normalization_and_rollup_10000(bencher: codspeed_divan_compat::Bencher) {
        assert_eq!(METRICS_OUTPUT.pods.len(), 10_007);
        assert_eq!(METRICS_OUTPUT.nodes.len(), 1);
        assert_eq!(METRICS_OUTPUT.workloads.len(), 10);

        let profile_replicaset = workload(&METRICS_OUTPUT, "ReplicaSet", "profile", "profile-rs");
        let profile_deployment = workload(
            &METRICS_OUTPUT,
            "Deployment",
            "profile",
            "profile-deployment",
        );
        let expected_profile_sources: Vec<_> = (0..9_999)
            .map(|index| format!("profile-pod-{index}"))
            .collect();
        for metric in [profile_replicaset, profile_deployment] {
            assert_eq!(metric.cpu_millicores, Some(100_045.0));
            assert_eq!(metric.memory_bytes, Some(10_485_235_712));
            assert_eq!(metric.sampled_at.as_deref(), Some("2026-01-01T00:10:00Z"));
            assert_eq!(metric.source_pods, expected_profile_sources);
        }

        let multi_container = pod_metric(&METRICS_OUTPUT, "profile", "profile-pod-0");
        assert_eq!(multi_container.cpu_millicores, Some(60.0));
        assert_eq!(multi_container.memory_bytes, Some(1_572_864));
        assert_eq!(
            multi_container.sampled_at.as_deref(),
            Some("2026-01-01T00:05:00Z")
        );
        let invalid_and_overflow = pod_metric(&METRICS_OUTPUT, "profile", "profile-pod-2");
        assert_eq!(invalid_and_overflow.cpu_millicores, Some(5.0));
        assert_eq!(invalid_and_overflow.memory_bytes, None);

        let direct_owner = workload(&METRICS_OUTPUT, "StatefulSet", "direct", "direct-ledger");
        assert_eq!(direct_owner.cpu_millicores, Some(100.0));
        assert_eq!(direct_owner.memory_bytes, Some(1_572_864));
        assert_eq!(
            direct_owner.sampled_at.as_deref(),
            Some("2026-01-01T00:03:00Z")
        );
        assert_eq!(
            direct_owner.source_pods,
            vec!["direct-early".to_string(), "direct-late".to_string()]
        );

        let tenant_a = workload(&METRICS_OUTPUT, "ReplicaSet", "tenant-a", "shared-rs");
        let tenant_a_deployment =
            workload(&METRICS_OUTPUT, "Deployment", "tenant-a", "shared-deploy");
        let tenant_b = workload(&METRICS_OUTPUT, "ReplicaSet", "tenant-b", "shared-rs");
        let tenant_b_deployment =
            workload(&METRICS_OUTPUT, "Deployment", "tenant-b", "shared-deploy");
        for metric in [tenant_a, tenant_a_deployment] {
            assert_eq!(metric.cpu_millicores, Some(100.0));
            assert_eq!(metric.memory_bytes, Some(1_048_576));
            assert_eq!(metric.source_pods, vec!["collision-pod".to_string()]);
        }
        for metric in [tenant_b, tenant_b_deployment] {
            assert_eq!(metric.cpu_millicores, Some(300.0));
            assert_eq!(metric.memory_bytes, Some(2_097_152));
            assert_eq!(metric.source_pods, vec!["collision-pod".to_string()]);
        }

        let overflow_replicaset =
            workload(&METRICS_OUTPUT, "ReplicaSet", "overflow", "overflow-rs");
        let overflow_deployment =
            workload(&METRICS_OUTPUT, "Deployment", "overflow", "overflow-deploy");
        for metric in [overflow_replicaset, overflow_deployment] {
            assert_eq!(metric.cpu_millicores, None);
            assert_eq!(metric.memory_bytes, None);
            assert_eq!(metric.sampled_at.as_deref(), Some("2026-01-01T00:50:00Z"));
            assert_eq!(
                metric.source_pods,
                vec!["overflow-a".to_string(), "overflow-b".to_string()]
            );
        }

        let unavailable = pod_metric(&METRICS_OUTPUT, "unavailable", "unavailable-pod");
        assert_eq!(unavailable.cpu_millicores, None);
        assert_eq!(unavailable.memory_bytes, None);
        let unavailable_owner = workload(
            &METRICS_OUTPUT,
            "StatefulSet",
            "unavailable",
            "unavailable-ledger",
        );
        assert_eq!(unavailable_owner.cpu_millicores, None);
        assert_eq!(unavailable_owner.memory_bytes, None);
        assert_eq!(
            unavailable_owner.source_pods,
            vec!["unavailable-pod".to_string()]
        );

        assert_eq!(METRICS_OUTPUT.nodes[0].cpu_millicores, Some(2_000.0));
        assert_eq!(METRICS_OUTPUT.nodes[0].memory_bytes, Some(2_147_483_648));
        bencher.bench(|| black_box(project_resource_metrics(black_box(&METRICS_INPUT))));
    }
}

mod resource_metrics_decode_and_rollup_10000 {
    use super::*;
    use k8s_openapi::api::{apps::v1::ReplicaSet, core::v1::Pod};
    use kubecove_inspection::commands::metrics::decode_and_roll_up_resource_metrics;
    use serde_json::json;

    type Fixture = (String, String, Vec<Pod>, Vec<ReplicaSet>);

    static INPUT: LazyLock<Fixture> = LazyLock::new(|| {
        let mut pods = Vec::new();
        let mut items = Vec::new();
        for index in 0..10_000 {
            let name = format!("pod-{index}");
            pods.push(serde_json::from_value(json!({
                "metadata": {"name":name,"namespace":"apps","ownerReferences":[
                    {"apiVersion":"apps/v1","kind":"ReplicaSet","name":"worker","uid":"rs","controller":true}
                ]}
            })).unwrap());
            items.push(json!({
                "metadata":{"name":name,"namespace":"apps","labels":{"app":"worker"}},
                "timestamp":"2026-01-01T00:00:00Z",
                "containers":[{"name":"app","usage":{"cpu":"10m","memory":"1Mi"}}]
            }));
        }
        let replica_set = serde_json::from_value(json!({
            "metadata":{"name":"worker","namespace":"apps","uid":"rs","ownerReferences":[
                {"apiVersion":"apps/v1","kind":"Deployment","name":"worker","uid":"deploy","controller":true}
            ]}
        })).unwrap();
        (
            json!({"metadata":{}, "items":items}).to_string(),
            json!({"metadata":{}, "items":[]}).to_string(),
            pods,
            vec![replica_set],
        )
    });

    fn project() -> ResourceMetricsSummary {
        let (pod_list, node_list, pods, replica_sets) = &*INPUT;
        decode_and_roll_up_resource_metrics("fixture", pod_list, node_list, pods, replica_sets)
            .expect("metrics list")
    }

    #[codspeed_divan_compat::bench]
    fn resource_metrics_decode_and_rollup_10000(bencher: codspeed_divan_compat::Bencher) {
        let output = project();
        assert_eq!(output.pods.len(), 10_000);
        assert_eq!(output.workloads.len(), 2);
        assert_eq!(
            workload(&output, "Deployment", "apps", "worker").memory_bytes,
            Some(10_485_760_000)
        );
        bencher.bench(|| black_box(project()));
    }
}
