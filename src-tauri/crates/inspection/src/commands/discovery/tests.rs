use super::*;
use http::{header::CONTENT_TYPE, Request, Response};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinitionVersion;
use kube::{client::Body, Client};
use serde_json::json;

type MockHandle = tower_test::mock::Handle<Request<Body>, Response<Body>>;

fn mock_client() -> (Client, MockHandle) {
    let (service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    (Client::new(service, "default"), handle)
}

fn response(status: u16, body: impl Into<Body>) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(body.into())
        .expect("response")
}

fn crd(name: &str, group: &str, kind: &str, plural: &str, scope: &str) -> CatalogCrd {
    let source = CustomResourceDefinition {
        metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta::default(),
        spec: k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinitionSpec {
            group: group.to_string(),
            names: k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinitionNames {
                kind: kind.to_string(),
                plural: plural.to_string(),
                ..Default::default()
            },
            scope: scope.to_string(),
            versions: vec![
                CustomResourceDefinitionVersion {
                    name: "v1beta1".to_string(),
                    served: true,
                    storage: false,
                    ..Default::default()
                },
                CustomResourceDefinitionVersion {
                    name: name.to_string(),
                    served: true,
                    storage: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
        status: None,
    };
    serde_json::from_value(serde_json::to_value(source).expect("CRD fixture"))
        .expect("catalog fixture")
}

#[test]
fn falls_back_to_first_served_version() {
    let mut source = crd("v1", "example.com", "Widget", "widgets", "Cluster");
    source.spec.versions[1].served = false;

    let kind = discovered_kind_from_crd(&source).expect("kind");

    assert_eq!(kind.version, "v1beta1");
    assert!(!kind.namespaced);
}

#[test]
fn ignores_crd_with_no_served_version() {
    let mut source = crd("v1", "example.com", "Widget", "widgets", "Namespaced");
    for version in &mut source.spec.versions {
        version.served = false;
    }

    assert!(discovered_kind_from_crd(&source).is_none());
}

#[test]
fn sorts_and_dedups_kinds() {
    let mut kinds = vec![
        discovered_kind_from_crd(&crd("v1", "z.example", "Zed", "zeds", "Namespaced")).unwrap(),
        discovered_kind_from_crd(&crd("v1", "a.example", "Alpha", "alphas", "Namespaced")).unwrap(),
        discovered_kind_from_crd(&crd("v1", "a.example", "Alpha", "alphas", "Namespaced")).unwrap(),
    ];

    sort_and_dedup_kinds(&mut kinds);

    assert_eq!(kinds.len(), 2);
    assert_eq!(kinds[0].kind, "Alpha");
    assert_eq!(kinds[1].kind, "Zed");
}

#[test]
fn keeps_cluster_scoped_kinds_for_all_namespace_presence() {
    let cluster_kind = discovered_kind_from_crd(&crd(
        "v1",
        "example.com",
        "ClusterThing",
        "clusterthings",
        "Cluster",
    ))
    .unwrap();
    let namespaced_kind =
        discovered_kind_from_crd(&crd("v1", "example.com", "Widget", "widgets", "Namespaced"))
            .unwrap();

    assert!(should_probe_present_kind(&cluster_kind, &[]));
    assert!(should_probe_present_kind(&namespaced_kind, &[]));
    assert!(!should_probe_present_kind(
        &cluster_kind,
        &[String::from("default")]
    ));
    assert!(should_probe_present_kind(
        &namespaced_kind,
        &[String::from("default")]
    ));
}

#[tokio::test]
async fn discovery_preserves_catalog_fields_with_large_unused_schema() {
    // Failure modes: dropping short names or scope, choosing an unserved
    // storage version, and changing the CRD endpoint or list parameters.
    let (client, mut handle) = mock_client();
    let mut source = crd("v1", "example.com", "Widget", "widgets", "Namespaced");
    source.spec.names.short_names = Some(vec!["wdg".into(), "wi".into()]);
    source.spec.versions[1].served = false;
    let expected = discovered_kind_from_crd(&source).expect("served kind");
    let mut source = json!({
        "apiVersion": "apiextensions.k8s.io/v1", "kind": "CustomResourceDefinition",
        "metadata": {}, "spec": {
            "group": source.spec.group, "names": source.spec.names,
            "scope": source.spec.scope,
            "versions": [{"name": "v1beta1", "served": true, "storage": false},
                         {"name": "v1", "served": false, "storage": true}]
        }
    });
    source["metadata"]["annotations"] = json!({"example.com/large": "x".repeat(65_536)});
    source["spec"]["versions"][0]["schema"] = json!({
        "openAPIV3Schema": {
            "type": "object",
            "properties": {"spec": {"type": "object", "description": "x".repeat(65_536)}}
        }
    });
    let operation = crd_resource_kinds(client);
    let responder = async move {
        let (request, send) = handle.next_request().await.expect("CRD list request");
        let expected_request =
            kube::core::Request::new("/apis/apiextensions.k8s.io/v1/customresourcedefinitions")
                .list(&list_params())
                .expect("list request");
        assert_eq!(request.uri(), expected_request.uri());
        send.send_response(response(
            200,
            Body::from(serde_json::to_vec(&json!({"items": [source]})).expect("list fixture")),
        ));
    };
    let (result, ()) = tokio::join!(operation, responder);
    assert_eq!(
        serde_json::to_value(result.expect("catalog")).unwrap(),
        json!([expected])
    );
}

#[tokio::test]
async fn malformed_catalog_field_is_reported() {
    // The projection must reject type metadata that the full CRD decoder rejects.
    for item in [
        json!({"spec": {"group": 123}}),
        json!({"apiVersion": "wrong/v1"}),
        json!({"kind": "Pod"}),
        json!({"kind": {"CustomResourceDefinition": null}}),
    ] {
        let (client, mut handle) = mock_client();
        let operation = crd_resource_kinds(client);
        let responder = async move {
            let (_, send) = handle.next_request().await.expect("CRD list request");
            send.send_response(response(
                200,
                Body::from(serde_json::to_vec(&json!({"items": [item]})).unwrap()),
            ));
        };
        let (result, ()) = tokio::join!(operation, responder);
        assert_eq!(
            result.expect_err("invalid group").kind,
            AppErrorKind::Serialization
        );
    }
}

#[tokio::test]
async fn malformed_discovery_response_is_reported() {
    for body in [b"{not-json".to_vec(), vec![0xff]] {
        let (client, mut handle) = mock_client();
        let operation = crd_resource_kinds(client);
        let responder = async move {
            let (_, send) = handle.next_request().await.expect("CRD list request");
            send.send_response(response(200, Body::from(body)));
        };

        let (result, ()) = tokio::join!(operation, responder);
        let error = result.expect_err("malformed response must fail");

        assert_eq!(error.kind, AppErrorKind::Serialization);
    }
}

#[tokio::test]
async fn forbidden_discovery_response_preserves_permission_kind() {
    let (client, mut handle) = mock_client();
    let operation = crd_resource_kinds(client);
    let responder = async move {
        let (_, send) = handle.next_request().await.expect("CRD list request");
        send.send_response(response(
            403,
            Body::from(
                serde_json::to_vec(&json!({
                    "apiVersion": "v1",
                    "kind": "Status",
                    "status": "Failure",
                    "reason": "Forbidden",
                    "message": "customresourcedefinitions is forbidden",
                    "code": 403
                }))
                .expect("status JSON"),
            ),
        ));
    };

    let (result, ()) = tokio::join!(operation, responder);
    let error = result.expect_err("forbidden response must fail");

    assert_eq!(error.kind, AppErrorKind::Forbidden);
    assert!(error.message.contains("forbidden"));
}
