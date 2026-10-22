use codspeed_divan_compat::{black_box, Bencher};
use k8s_openapi::api::core::v1::Pod;
use kube::api::ObjectList;
use kubecove_inspection::commands::resources::{
    builtin_summary, decode_pod_list_summaries, decode_summary_projection,
    encode_shared_scope_rows, resource_scope_post_fetch,
};
use std::sync::Arc;
use std::sync::LazyLock;

static SCOPE: LazyLock<Vec<Arc<Vec<kubecove_models::ResourceSummary>>>> = LazyLock::new(|| {
    kubecove_lib::commands::bench_support::resource_catalog::resource_scope_fixture()
        .batches
        .into_iter()
        .map(Arc::new)
        .collect()
});

#[codspeed_divan_compat::bench]
fn scope_cached_rows_owned_encoding(bencher: Bencher) {
    let fixture = kubecove_lib::commands::bench_support::resource_catalog::resource_scope_fixture();
    let index = kubecove_kube::commands::helpers::FluxOwnershipIndex::default();
    let expected = resource_scope_post_fetch(
        fixture.requests.clone(),
        SCOPE.iter().map(|batch| batch.as_ref().clone()).collect(),
        index.clone(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&expected).unwrap(),
        encode_shared_scope_rows(SCOPE.clone(), &index).unwrap()
    );
    bencher.bench(|| {
        let rows = resource_scope_post_fetch(
            fixture.requests.clone(),
            SCOPE.iter().map(|batch| batch.as_ref().clone()).collect(),
            index.clone(),
        )
        .unwrap();
        black_box(serde_json::to_string(&rows).unwrap())
    });
}

#[codspeed_divan_compat::bench]
fn scope_cached_rows_shared_encoding(bencher: Bencher) {
    let _ = &*SCOPE;
    let index = kubecove_kube::commands::helpers::FluxOwnershipIndex::default();
    bencher.bench(|| black_box(encode_shared_scope_rows(SCOPE.clone(), &index).unwrap()));
}

// Synthetic allocation workload, not a claim about any particular cluster.
static PODS: LazyLock<String> = LazyLock::new(|| {
    let spec = serde_json::json!({"containers":[{"name":"app", "image":"app:v1",
        "env":(0..32).map(|i| serde_json::json!({"name":format!("SETTING_{i}"), "value":"configured-value"})).collect::<Vec<_>>() }]});
    let applied = serde_json::to_string(&spec).unwrap();
    serde_json::json!({"apiVersion":"v1", "kind":"PodList", "metadata":{"resourceVersion":"1"},
        "items":(0..1_000).map(|i| serde_json::json!({"apiVersion":"v1", "kind":"Pod",
            "metadata":{"name":format!("pod-{i}"), "namespace":"tenant", "creationTimestamp":"2026-10-01T00:00:00Z",
                "annotations":{"kubectl.kubernetes.io/last-applied-configuration":applied},
                "managedFields":[{"manager":"controller", "fieldsType":"FieldsV1", "fieldsV1":{"f:spec":{"f:containers":{}}}}]},
            "spec":spec, "status":{"phase":"Running", "conditions":[{"type":"Ready", "status":"True"}],
                "containerStatuses":[{"name":"app", "image":"app:v1", "imageID":"id", "ready":true, "restartCount":2}]}
        })).collect::<Vec<_>>()}).to_string()
});

const KINDS: [&str; 5] = ["Deployment", "ReplicaSet", "Job", "Event", "Widget"];
static OTHER_LISTS: LazyLock<Vec<String>> = LazyLock::new(|| {
    KINDS
        .iter()
        .map(|kind| {
            let mut list: serde_json::Value = serde_json::from_str(&PODS).unwrap();
            for item in list["items"].as_array_mut().unwrap() {
                item["kind"] = serde_json::json!(kind);
                item["apiVersion"] = serde_json::json!(match *kind {
                    "Deployment" | "ReplicaSet" => "apps/v1",
                    "Job" => "batch/v1",
                    "Widget" => "example.com/v1",
                    _ => "v1",
                });
                if matches!(*kind, "Deployment" | "ReplicaSet" | "Job") {
                    let pod_spec = item["spec"].take();
                    item["spec"] = serde_json::json!({"replicas":3, "completions":3,
                        "selector":{"matchLabels":{"app":"app"}},
                        "template":{"metadata":{"labels":{"app":"app"}}, "spec":pod_spec}});
                    item["status"] = serde_json::json!({"replicas":3, "readyReplicas":2,
                        "availableReplicas":2, "active":1, "succeeded":2, "failed":0});
                }
                if *kind == "Event" {
                    item.as_object_mut().unwrap().remove("spec");
                    item.as_object_mut().unwrap().remove("status");
                    item["metadata"]
                        .as_object_mut()
                        .unwrap()
                        .remove("annotations");
                    item["involvedObject"] =
                        serde_json::json!({"kind":"Pod", "name":"app", "namespace":"tenant"});
                    item["type"] = serde_json::json!("Warning");
                    item["message"] = serde_json::json!("Container failed to start");
                }
            }
            list.to_string()
        })
        .collect()
});

#[codspeed_divan_compat::bench(args = KINDS)]
fn list_decode_full_1000(bencher: Bencher, kind: &str) {
    let text = &OTHER_LISTS[KINDS.iter().position(|k| *k == kind).unwrap()];
    macro_rules! decode {
        ($resource:ty) => {{
            let list: ObjectList<$resource> = serde_json::from_str(black_box(text)).unwrap();
            black_box(list).items.len()
        }};
    }
    bencher.bench(|| {
        black_box(match kind {
            "Deployment" => decode!(k8s_openapi::api::apps::v1::Deployment),
            "ReplicaSet" => decode!(k8s_openapi::api::apps::v1::ReplicaSet),
            "Job" => decode!(k8s_openapi::api::batch::v1::Job),
            "Event" => decode!(k8s_openapi::api::core::v1::Event),
            "Widget" => decode!(kube::api::DynamicObject),
            _ => unreachable!(),
        })
    });
}

#[codspeed_divan_compat::bench(args = KINDS)]
fn list_decode_projected_1000(bencher: Bencher, kind: &str) {
    let text = &OTHER_LISTS[KINDS.iter().position(|k| *k == kind).unwrap()];
    assert_eq!(decode_summary_projection(kind, text).unwrap(), 1_000);
    bencher.bench(|| black_box(decode_summary_projection(kind, black_box(text)).unwrap()));
}

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-10-21T00:00:00Z")
        .unwrap()
        .to_utc()
}

fn full() -> Vec<kubecove_models::ResourceSummary> {
    let clock = now();
    let list: ObjectList<Pod> = serde_json::from_str(&PODS).unwrap();
    list.items
        .iter()
        .map(|pod| builtin_summary("benchmark", pod, clock))
        .collect()
}

#[codspeed_divan_compat::bench]
fn pod_list_decode_full_1000(bencher: Bencher) {
    assert_eq!(
        serde_json::to_vec(&full()).unwrap(),
        serde_json::to_vec(&decode_pod_list_summaries(&PODS, now()).unwrap()).unwrap()
    );
    bencher.bench(|| black_box(full()));
}

#[codspeed_divan_compat::bench]
fn pod_list_decode_projected_1000(bencher: Bencher) {
    let _ = &*PODS;
    let clock = now();
    bencher.bench(|| black_box(decode_pod_list_summaries(black_box(&PODS), clock).unwrap()));
}
