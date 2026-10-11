use super::*;
use std::collections::HashSet;

fn node_id<'a>(topology: &'a ResourceTopology, kind: &str, namespace: &str, name: &str) -> &'a str {
    let mut nodes = topology.nodes.iter().filter(|node| {
        node.kind == kind && node.namespace.as_deref() == Some(namespace) && node.name == name
    });
    let node = nodes.next().expect("expected topology node");
    assert!(nodes.next().is_none(), "topology identity must be unique");
    &node.id
}

fn has_edge(
    topology: &ResourceTopology,
    source: &str,
    target: &str,
    relation: &kubecove_lib::models::TopologyRelation,
) -> bool {
    topology
        .edges
        .iter()
        .any(|edge| edge.source == source && edge.target == target && &edge.relation == relation)
}

fn topology_kind_rank(kind: &str) -> usize {
    match kind {
        "Deployment" | "DaemonSet" | "StatefulSet" | "CronJob" | "Service" | "Ingress"
        | "ConfigMap" | "Secret" => 0,
        "ReplicaSet" | "Job" => 1,
        "EndpointSlice" | "Pod" => 2,
        "PersistentVolumeClaim" => 3,
        _ => 4,
    }
}

fn topology_order_key(node: &kubecove_lib::models::TopologyNode) -> (usize, Option<&str>, &str) {
    (
        topology_kind_rank(&node.kind),
        node.namespace.as_deref(),
        &node.name,
    )
}

fn assert_sorted_unique(topology: &ResourceTopology) {
    let mut node_ids = HashSet::with_capacity(topology.nodes.len());
    assert!(topology
        .nodes
        .iter()
        .all(|node| node_ids.insert(node.id.as_str())));
    assert!(topology
        .nodes
        .windows(2)
        .all(|pair| topology_order_key(&pair[0]) <= topology_order_key(&pair[1])));
    assert!(topology
        .edges
        .windows(2)
        .all(|pair| pair[0].id < pair[1].id));
}

// R08, folded R07: complete typed/dynamic mapping, labels, health, ownership, StatefulSet PVCs.
mod ownership_topology_production_mapping_500_apps {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn ownership_topology_production_mapping_500_apps(bencher: codspeed_divan_compat::Bencher) {
        assert_eq!(OWNERSHIP_OUTPUT.nodes.len(), 2_512);
        assert_sorted_unique(&OWNERSHIP_OUTPUT);
        assert!(OWNERSHIP_OUTPUT
            .edges
            .iter()
            .any(|edge| edge.relation == kubecove_lib::models::TopologyRelation::Creates));
        assert!(OWNERSHIP_OUTPUT
            .nodes
            .iter()
            .any(|node| node.kind == "CronJob"));
        assert!(OWNERSHIP_OUTPUT
            .nodes
            .iter()
            .any(|node| node.kind == "Node"));
        assert!(!OWNERSHIP_OUTPUT
            .nodes
            .iter()
            .any(|node| node.name == "kube-root-ca.crt"));
        assert!(!OWNERSHIP_OUTPUT.warnings.is_empty());

        let replica_set_id = node_id(
            &OWNERSHIP_OUTPUT,
            "ReplicaSet",
            "namespace-0",
            "checkout-0-rs",
        );
        let decoy_replica_set_id = node_id(
            &OWNERSHIP_OUTPUT,
            "ReplicaSet",
            "namespace-collision",
            "checkout-0-rs",
        );
        let checkout_pod_id = node_id(&OWNERSHIP_OUTPUT, "Pod", "namespace-0", "checkout-0-pod");
        assert!(has_edge(
            &OWNERSHIP_OUTPUT,
            replica_set_id,
            checkout_pod_id,
            &kubecove_lib::models::TopologyRelation::Owns,
        ));
        assert!(!has_edge(
            &OWNERSHIP_OUTPUT,
            decoy_replica_set_id,
            checkout_pod_id,
            &kubecove_lib::models::TopologyRelation::Owns,
        ));

        let stateful_set_id = node_id(&OWNERSHIP_OUTPUT, "StatefulSet", "tenant-ledger", "ledger");
        let claim_id = node_id(
            &OWNERSHIP_OUTPUT,
            "PersistentVolumeClaim",
            "tenant-ledger",
            "data-ledger-0",
        );
        let shadow_stateful_set_id =
            node_id(&OWNERSHIP_OUTPUT, "StatefulSet", "tenant-shadow", "ledger");
        let shadow_claim_id = node_id(
            &OWNERSHIP_OUTPUT,
            "PersistentVolumeClaim",
            "tenant-shadow",
            "data-ledger-0",
        );
        assert!(has_edge(
            &OWNERSHIP_OUTPUT,
            stateful_set_id,
            claim_id,
            &kubecove_lib::models::TopologyRelation::Creates,
        ));
        assert!(has_edge(
            &OWNERSHIP_OUTPUT,
            shadow_stateful_set_id,
            shadow_claim_id,
            &kubecove_lib::models::TopologyRelation::Creates,
        ));
        assert!(!has_edge(
            &OWNERSHIP_OUTPUT,
            stateful_set_id,
            shadow_claim_id,
            &kubecove_lib::models::TopologyRelation::Creates,
        ));
        assert!(!has_edge(
            &OWNERSHIP_OUTPUT,
            shadow_stateful_set_id,
            claim_id,
            &kubecove_lib::models::TopologyRelation::Creates,
        ));

        bencher
            .with_inputs(|| OWNERSHIP_INPUT.clone())
            .bench_values(|fixture| black_box(build_ownership_topology(fixture)));
    }
}

// R09, folded R10: production mapping plus EndpointSlice precedence and selector fallback.
mod network_topology_endpoint_slice_and_selector_500_services {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn network_topology_endpoint_slice_and_selector_500_services(
        bencher: codspeed_divan_compat::Bencher,
    ) {
        assert_eq!(NETWORK_OUTPUT.nodes.len(), 1_255);
        assert_sorted_unique(&NETWORK_OUTPUT);
        assert!(!NETWORK_OUTPUT.warnings.is_empty());

        let ingress_id = node_id(&NETWORK_OUTPUT, "Ingress", "tenant-0", "ledger-public");
        let service_id = node_id(&NETWORK_OUTPUT, "Service", "tenant-0", "ledger-0-service");
        let same_name_service_id =
            node_id(&NETWORK_OUTPUT, "Service", "tenant-1", "ledger-0-service");
        assert!(has_edge(
            &NETWORK_OUTPUT,
            ingress_id,
            service_id,
            &kubecove_lib::models::TopologyRelation::RoutesTo,
        ));
        assert!(!has_edge(
            &NETWORK_OUTPUT,
            ingress_id,
            same_name_service_id,
            &kubecove_lib::models::TopologyRelation::RoutesTo,
        ));

        let missing_target_slice_id = node_id(
            &NETWORK_OUTPUT,
            "EndpointSlice",
            "tenant-0",
            "ledger-0-service-slice",
        );
        let matched_pod_id = node_id(&NETWORK_OUTPUT, "Pod", "tenant-0", "ledger-0");
        assert!(has_edge(
            &NETWORK_OUTPUT,
            service_id,
            missing_target_slice_id,
            &kubecove_lib::models::TopologyRelation::Targets,
        ));
        assert!(!has_edge(
            &NETWORK_OUTPUT,
            service_id,
            matched_pod_id,
            &kubecove_lib::models::TopologyRelation::Selects,
        ));
        assert!(!NETWORK_OUTPUT.edges.iter().any(|edge| {
            edge.source == missing_target_slice_id
                && edge.relation == kubecove_lib::models::TopologyRelation::Targets
        }));
        assert!(NETWORK_OUTPUT
            .warnings
            .iter()
            .any(|warning| warning
                == "Service tenant-0/ledger-0-service has no matching Pod endpoints"));
        let missing_target_slice = NETWORK_OUTPUT
            .nodes
            .iter()
            .find(|node| node.id == missing_target_slice_id)
            .expect("attached EndpointSlice node");
        assert!(!missing_target_slice.selectable);

        let duplicate_target_slice_id = node_id(
            &NETWORK_OUTPUT,
            "EndpointSlice",
            "tenant-1",
            "ledger-1-service-slice",
        );
        let tenant_one_pod_id = node_id(&NETWORK_OUTPUT, "Pod", "tenant-1", "ledger-1");
        let tenant_two_pod_id = node_id(&NETWORK_OUTPUT, "Pod", "tenant-2", "ledger-1");
        assert!(has_edge(
            &NETWORK_OUTPUT,
            duplicate_target_slice_id,
            tenant_one_pod_id,
            &kubecove_lib::models::TopologyRelation::Targets,
        ));
        assert!(!has_edge(
            &NETWORK_OUTPUT,
            duplicate_target_slice_id,
            tenant_two_pod_id,
            &kubecove_lib::models::TopologyRelation::Targets,
        ));
        assert_eq!(
            NETWORK_OUTPUT
                .edges
                .iter()
                .filter(|edge| {
                    edge.source == duplicate_target_slice_id
                        && edge.target == tenant_one_pod_id
                        && edge.relation == kubecove_lib::models::TopologyRelation::Targets
                })
                .count(),
            1
        );

        let collision_slice_id = node_id(
            &NETWORK_OUTPUT,
            "EndpointSlice",
            "tenant-1",
            "ledger-0-service-collision-slice",
        );
        assert!(has_edge(
            &NETWORK_OUTPUT,
            same_name_service_id,
            collision_slice_id,
            &kubecove_lib::models::TopologyRelation::Targets,
        ));
        assert!(has_edge(
            &NETWORK_OUTPUT,
            collision_slice_id,
            tenant_one_pod_id,
            &kubecove_lib::models::TopologyRelation::Targets,
        ));
        assert!(!has_edge(
            &NETWORK_OUTPUT,
            collision_slice_id,
            tenant_two_pod_id,
            &kubecove_lib::models::TopologyRelation::Targets,
        ));
        assert!(!has_edge(
            &NETWORK_OUTPUT,
            service_id,
            collision_slice_id,
            &kubecove_lib::models::TopologyRelation::Targets,
        ));
        assert!(NETWORK_OUTPUT.warnings.iter().any(|warning| warning
            == "Ingress tenant-0/ledger-public routes to missing Service/missing-service"));
        assert!(NETWORK_OUTPUT.warnings.iter().any(|warning| warning
            == "EndpointSlice tenant-special/unlabelled-slice has no service-name label"));

        let fallback_service_id = node_id(
            &NETWORK_OUTPUT,
            "Service",
            "tenant-50",
            "ledger-250-service",
        );
        let actual_fallback_targets: HashSet<_> = NETWORK_OUTPUT
            .edges
            .iter()
            .filter(|edge| {
                edge.source == fallback_service_id
                    && edge.relation == kubecove_lib::models::TopologyRelation::Selects
            })
            .map(|edge| edge.target.as_str())
            .collect();
        let expected_fallback_targets: HashSet<_> = [50, 150, 250, 350, 450]
            .into_iter()
            .map(|index| {
                node_id(
                    &NETWORK_OUTPUT,
                    "Pod",
                    "tenant-50",
                    &format!("ledger-{index}"),
                )
            })
            .collect();
        assert_eq!(actual_fallback_targets, expected_fallback_targets);

        let external_name_id = node_id(
            &NETWORK_OUTPUT,
            "Service",
            "tenant-special",
            "external-database",
        );
        assert!(!NETWORK_OUTPUT
            .edges
            .iter()
            .any(|edge| edge.source == external_name_id));

        bencher
            .with_inputs(|| NETWORK_INPUT.clone())
            .bench_values(|fixture| black_box(build_network_topology(fixture)));
    }
}
