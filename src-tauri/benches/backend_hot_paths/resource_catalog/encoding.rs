use super::*;

// R48: production JSON String response encoding for complete domain outputs.
mod json_encode_typed_resource_summaries {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_typed_resource_summaries(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*TYPED_OUTPUT).expect("typed response JSON");
        assert!(serde_json::from_str::<serde_json::Value>(&encoded)
            .unwrap()
            .is_array());
        bencher
            .with_inputs(|| TYPED_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("typed response JSON"))
            });
    }
}

mod json_encode_dynamic_resource_summaries {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_dynamic_resource_summaries(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*DYNAMIC_OUTPUT).expect("dynamic response JSON");
        assert!(serde_json::from_str::<serde_json::Value>(&encoded)
            .unwrap()
            .is_array());
        bencher
            .with_inputs(|| DYNAMIC_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("dynamic response JSON"))
            });
    }
}

mod json_encode_ownership_topology {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_ownership_topology(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*OWNERSHIP_OUTPUT).expect("topology response JSON");
        assert!(serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["nodes"].is_array());
        bencher
            .with_inputs(|| OWNERSHIP_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("topology response JSON"))
            });
    }
}

mod json_encode_network_topology {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_network_topology(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*NETWORK_OUTPUT).expect("network topology JSON");
        assert!(serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["edges"].is_array());
        bencher
            .with_inputs(|| NETWORK_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("network topology JSON"))
            });
    }
}

mod json_encode_resource_metrics {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_resource_metrics(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*METRICS_OUTPUT).expect("metrics response JSON");
        assert!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["workloads"].is_array()
        );
        bencher
            .with_inputs(|| METRICS_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("metrics response JSON"))
            });
    }
}

mod json_encode_incident_cockpit {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_incident_cockpit(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*INCIDENT_OUTPUT).expect("incident response JSON");
        assert!(serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["items"].is_array());
        bencher
            .with_inputs(|| INCIDENT_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("incident response JSON"))
            });
    }
}

mod json_encode_selected_resource_events {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_selected_resource_events(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*EVENT_OUTPUT).expect("event response JSON");
        assert!(serde_json::from_str::<serde_json::Value>(&encoded)
            .unwrap()
            .is_array());
        bencher
            .with_inputs(|| EVENT_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("event response JSON"))
            });
    }
}

// R48 variants for the remaining complete response-producing workloads.
mod json_encode_namespace_summaries {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_namespace_summaries(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*NAMESPACE_OUTPUT).expect("namespace response JSON");
        assert!(serde_json::from_str::<serde_json::Value>(&encoded)
            .unwrap()
            .is_array());
        bencher
            .with_inputs(|| NAMESPACE_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("namespace response JSON"))
            });
    }
}

mod json_encode_resource_scope_rows {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_resource_scope_rows(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*SCOPE_OUTPUT).expect("scope response JSON");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded)
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2_000
        );
        bencher
            .with_inputs(|| SCOPE_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("scope response JSON"))
            });
    }
}

mod json_encode_crd_catalog {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_crd_catalog(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*CRD_CATALOG_OUTPUT).expect("catalog response JSON");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded)
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1_000
        );
        bencher
            .with_inputs(|| CRD_CATALOG_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("catalog response JSON"))
            });
    }
}

mod json_encode_kubeconfig_contexts {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_kubeconfig_contexts(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*KUBECONFIG_OUTPUT).expect("context response JSON");
        assert!(serde_json::from_str::<serde_json::Value>(&encoded)
            .unwrap()
            .is_array());
        bencher
            .with_inputs(|| KUBECONFIG_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("context response JSON"))
            });
    }
}

mod json_encode_deployment_details {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_deployment_details(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*DEPLOYMENT_DETAILS_OUTPUT).expect("details JSON");
        assert!(serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["yaml"].is_string());
        bencher
            .with_inputs(|| DEPLOYMENT_DETAILS_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("details JSON"))
            });
    }
}

mod json_encode_configmap_details {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_configmap_details(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*CONFIGMAP_DETAILS_OUTPUT).expect("details JSON");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["summary"]["name"],
            "checkout-config"
        );
        bencher
            .with_inputs(|| CONFIGMAP_DETAILS_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("details JSON"))
            });
    }
}

mod json_encode_secret_details {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_secret_details(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*SECRET_DETAILS_OUTPUT).expect("details JSON");
        assert!(!encoded.contains("c2VjcmV0"));
        bencher
            .with_inputs(|| SECRET_DETAILS_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("details JSON"))
            });
    }
}

mod json_encode_crd_details {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_crd_details(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*CRD_DETAILS_OUTPUT).expect("details JSON");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["summary"]["kind"],
            "CustomResourceDefinition"
        );
        bencher
            .with_inputs(|| CRD_DETAILS_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("details JSON"))
            });
    }
}

mod json_encode_dynamic_details {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_dynamic_details(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*DYNAMIC_DETAILS_OUTPUT).expect("details JSON");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["summary"]["kind"],
            "ManagedThing"
        );
        bencher
            .with_inputs(|| DYNAMIC_DETAILS_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("details JSON"))
            });
    }
}

mod json_encode_deployment_revisions {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_deployment_revisions(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*REVISION_OUTPUT).expect("revision response JSON");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap(),
            serde_json::to_value(&*REVISION_OUTPUT).expect("revision response value")
        );
        bencher
            .with_inputs(|| REVISION_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("revision response JSON"))
            });
    }
}

mod json_encode_editor_lint_result {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn json_encode_editor_lint_result(bencher: codspeed_divan_compat::Bencher) {
        let encoded = serde_json::to_string(&*EDITOR_LINT_OUTPUT).expect("lint response JSON");
        assert!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["diagnostics"].is_array()
        );
        bencher
            .with_inputs(|| EDITOR_LINT_OUTPUT.clone())
            .bench_values(|output| {
                black_box(serde_json::to_string(&output).expect("lint response JSON"))
            });
    }
}
