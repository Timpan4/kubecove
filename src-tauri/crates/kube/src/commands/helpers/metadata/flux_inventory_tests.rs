use super::*;
use http::{Request, Response, header::CONTENT_TYPE};
use kube::client::Body;
use serde_json::json;

type MockHandle = tower_test::mock::Handle<Request<Body>, Response<Body>>;

fn mock_client() -> (Client, MockHandle) {
    let (service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    (Client::new(service, "default"), handle)
}

fn json_response(status: u16, body: serde_json::Value) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn inventory_list() -> serde_json::Value {
    json!({
        "apiVersion": "v1",
        "items": [{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {"name": "api", "namespace": "default"},
            "status": {
                "inventory": {
                    "entries": [{"id": "default_api_apps_Deployment", "version": "v1"}]
                }
            }
        }]
    })
}

#[tokio::test]
async fn provider_list_failure_preserves_evidence_from_next_kind() {
    let (client, mut handle) = mock_client();
    let read = read_flux_ownership_index(client, &[]);
    let respond = async move {
        let (_, send) = handle.next_request().await.expect("Kustomization request");
        send.send_response(json_response(
            500,
            json!({
                "apiVersion": "v1", "kind": "Status", "message": "provider unavailable"
            }),
        ));

        let (_, send) = handle.next_request().await.expect("HelmRelease request");
        send.send_response(json_response(200, inventory_list()));
    };

    let (index, ()) = tokio::join!(read, respond);
    let index = index.expect("partial index");
    assert!(index.partial);
    assert!(index.owners.contains_key(&ResourceKey {
        namespace: Some("default".into()),
        api_version: "apps/v1".into(),
        kind: "Deployment".into(),
        name: "api".into(),
    }));
}

#[tokio::test]
async fn cancelled_provider_list_failure_is_fatal() {
    let (client, mut handle) = mock_client();
    let read = read_flux_ownership_index(client, &[]);
    let respond = async move {
        let (_, send) = handle.next_request().await.expect("Kustomization request");
        send.send_error(crate::models::WorkspaceRequestCancelled);
    };

    let (result, ()) = tokio::join!(read, respond);
    assert_eq!(
        result.expect_err("cancellation should be fatal").kind,
        AppErrorKind::Cancelled
    );
}

#[test]
fn continuation_token_marks_inventory_partial() {
    assert!(has_continuation_token(Some("next-page")));
    assert!(!has_continuation_token(Some("")));
    assert!(!has_continuation_token(None));
}

#[test]
fn parses_defensively() {
    assert_eq!(
        parse_inventory_id("default_api_apps_Deployment"),
        Some((
            Some("default".into()),
            "api".into(),
            "apps".into(),
            "Deployment".into(),
        ))
    );
    assert_eq!(
        parse_inventory_id("_shared_storage.k8s.io_StorageClass"),
        Some((
            None,
            "shared".into(),
            "storage.k8s.io".into(),
            "StorageClass".into(),
        ))
    );
    assert_eq!(parse_inventory_id("bad"), None);
    assert_eq!(parse_inventory_id("__apps_Deployment"), None);
}

#[test]
fn complete_inventory_is_not_partial_and_coverage_marks_unmatched_rows() {
    let mut index = FluxOwnershipIndex::default();
    index.owners.insert(
        ResourceKey {
            namespace: Some("default".into()),
            api_version: "apps/v1".into(),
            kind: "Deployment".into(),
            name: "api".into(),
        },
        GitOpsOwnerSummary {
            provider: "flux".into(),
            kind: "Kustomization".into(),
            name: "apps".into(),
            namespace: Some("flux-system".into()),
            confidence: "inventory".into(),
            provenance: "flux-inventory".into(),
            partial: false,
        },
    );
    let mut summaries = vec![summary("Deployment", "api"), summary("Service", "missing")];

    assert!(index.enriched_summary(&summaries[0], &[]).is_some());
    assert!(index.enriched_summary(&summaries[1], &[]).is_none());

    enrich_resource_summaries_with_flux_inventory(&mut summaries, &index);

    assert!(!summaries[0].git_ops_owner.as_ref().unwrap().partial);
    assert_eq!(summaries[1].git_ops_owner, None);
    assert!(!summaries.iter().any(|row| row.git_ops_ownership_partial));
    assert!(
        summaries
            .iter()
            .all(|row| index.enriched_summary(row, &[]).is_none())
    );

    index.partial = true;
    assert!(
        summaries
            .iter()
            .all(|row| index.enriched_summary(row, &[]).is_some())
    );
    enrich_resource_summaries_with_flux_inventory(&mut summaries, &index);
    assert!(summaries.iter().all(|row| row.git_ops_ownership_partial));
    assert!(summaries[0].git_ops_owner.as_ref().unwrap().partial);
    assert!(
        summaries
            .iter()
            .all(|row| index.enriched_summary(row, &[]).is_none())
    );
}

fn summary(kind: &str, name: &str) -> ResourceSummary {
    let mut summary: ResourceSummary = serde_json::from_value(json!({
        "kind": kind,
        "cluster": "kind-dev",
        "name": name,
        "namespace": "default",
        "age": "1m",
        "health": "unknown"
    }))
    .expect("summary");
    summary.api_version = Some(
        match kind {
            "Deployment" => "apps/v1",
            _ => "v1",
        }
        .to_string(),
    );
    summary
}

#[test]
fn api_version_is_part_of_inventory_identity() {
    let owner = GitOpsOwnerSummary {
        provider: "flux".into(),
        kind: "Kustomization".into(),
        name: "apps".into(),
        namespace: Some("flux-system".into()),
        confidence: "inventory".into(),
        provenance: "flux-inventory".into(),
        partial: false,
    };
    let mut index = FluxOwnershipIndex::default();
    index.owners.insert(
        ResourceKey {
            namespace: Some("default".into()),
            api_version: "apps/v1".into(),
            kind: "Deployment".into(),
            name: "api".into(),
        },
        owner,
    );
    let mut summaries = vec![summary("Deployment", "api")];
    enrich_resource_summaries_with_flux_inventory(&mut summaries, &index);
    assert!(summaries[0].git_ops_owner.is_some());

    summaries[0].api_version = Some("example.com/v1".into());
    summaries[0].git_ops_owner = None;
    enrich_resource_summaries_with_flux_inventory(&mut summaries, &index);
    assert!(summaries[0].git_ops_owner.is_none());
}

#[test]
fn core_and_incomplete_inventory_versions_are_handled_safely() {
    assert_eq!(normalized_api_version("", Some("v1")), Some("v1".into()));
    assert_eq!(
        normalized_api_version("apps", Some("v1")),
        Some("apps/v1".into())
    );
    assert_eq!(normalized_api_version("apps", None), None);
    assert_eq!(normalized_api_version("apps", Some("")), None);

    let mut index = FluxOwnershipIndex::default();
    let parsed = parse_inventory_id("default_config__ConfigMap").unwrap();
    let api_version = normalized_api_version(&parsed.2, Some("v1")).unwrap();
    index.owners.insert(
        ResourceKey {
            namespace: parsed.0,
            api_version,
            kind: parsed.3,
            name: parsed.1,
        },
        GitOpsOwnerSummary {
            provider: "flux".into(),
            kind: "Kustomization".into(),
            name: "apps".into(),
            namespace: None,
            confidence: "inventory".into(),
            provenance: "flux-inventory".into(),
            partial: false,
        },
    );
    let mut summaries = vec![summary("ConfigMap", "config")];
    enrich_resource_summaries_with_flux_inventory(&mut summaries, &index);
    assert!(summaries[0].git_ops_owner.is_some());
}
#[test]
fn cluster_scoped_inventory_survives_namespace_filtering_and_enriches_summary() {
    let owner = GitOpsOwnerSummary {
        provider: "flux".into(),
        kind: "Kustomization".into(),
        name: "apps".into(),
        namespace: Some("flux-system".into()),
        confidence: "inventory".into(),
        provenance: "flux-inventory".into(),
        partial: false,
    };
    let mut index = FluxOwnershipIndex::default();
    index.owners.insert(
        ResourceKey {
            namespace: None,
            api_version: "storage.k8s.io/v1".into(),
            kind: "StorageClass".into(),
            name: "shared".into(),
        },
        owner.clone(),
    );
    index.owners.insert(
        ResourceKey {
            namespace: Some("default".into()),
            api_version: "apps/v1".into(),
            kind: "Deployment".into(),
            name: "api".into(),
        },
        owner.clone(),
    );
    index.owners.insert(
        ResourceKey {
            namespace: Some("other".into()),
            api_version: "apps/v1".into(),
            kind: "Deployment".into(),
            name: "api".into(),
        },
        owner.clone(),
    );

    let requested = ["default".to_string()];
    let allowed = summary("Deployment", "api");
    let cluster = summary_cluster_scoped("StorageClass", "shared");
    let mut excluded = allowed.clone();
    excluded.namespace = Some("other".into());
    for row in [&allowed, &cluster] {
        assert_eq!(
            index
                .enriched_summary(row, &requested)
                .unwrap()
                .git_ops_owner,
            Some(owner.clone())
        );
    }
    assert!(index.enriched_summary(&excluded, &requested).is_none());

    let index = filter_flux_ownership_index(index, &requested);
    assert!(index.owners.contains_key(&ResourceKey {
        namespace: None,
        api_version: "storage.k8s.io/v1".into(),
        kind: "StorageClass".into(),
        name: "shared".into(),
    }));
    assert!(index.owners.contains_key(&ResourceKey {
        namespace: Some("default".into()),
        api_version: "apps/v1".into(),
        kind: "Deployment".into(),
        name: "api".into(),
    }));
    assert!(!index.owners.contains_key(&ResourceKey {
        namespace: Some("other".into()),
        api_version: "apps/v1".into(),
        kind: "Deployment".into(),
        name: "api".into(),
    }));

    let mut summaries = vec![cluster];
    enrich_resource_summaries_with_flux_inventory(&mut summaries, &index);

    assert_eq!(summaries[0].git_ops_owner, Some(owner));
}

fn summary_cluster_scoped(kind: &str, name: &str) -> ResourceSummary {
    let mut summary: ResourceSummary = serde_json::from_value(json!({
        "kind": kind,
        "cluster": "kind-dev",
        "name": name,
        "namespace": null,
        "age": "1m",
        "health": "unknown"
    }))
    .expect("summary");
    summary.api_version = Some(
        match kind {
            "StorageClass" => "storage.k8s.io/v1",
            _ => "v1",
        }
        .to_string(),
    );
    summary
}
