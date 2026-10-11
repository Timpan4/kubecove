use super::{fixed_now, summaries};
use crate::{
    commands::{helpers, resources},
    models::{AppError, DiscoveredResourceKind, ResourceListRequest, ResourceSummary},
};
use kube::api::DynamicObject;
use serde_json::json;

#[derive(Clone)]
pub struct FluxIndexFixture {
    batches: Vec<(String, Vec<DynamicObject>)>,
}

#[derive(Clone, Debug)]
pub struct FluxIndexProjection {
    pub retained_items: usize,
    pub partial: bool,
}

#[derive(Clone)]
pub struct ResourceScopeFixture {
    pub requests: Vec<ResourceListRequest>,
    pub batches: Vec<Vec<ResourceSummary>>,
    index: helpers::FluxOwnershipIndex,
}

pub fn flux_index_fixture() -> FluxIndexFixture {
    let mut batches = Vec::new();
    for (kind, group) in [
        ("Kustomization", "kustomize.toolkit.fluxcd.io"),
        ("HelmRelease", "helm.toolkit.fluxcd.io"),
    ] {
        let mut owners = Vec::with_capacity(helpers::MAX_FLUX_OWNER_OBJECTS);
        for owner_index in 0..helpers::MAX_FLUX_OWNER_OBJECTS {
            let entries = if kind == "Kustomization" && owner_index == 0 {
                (0..helpers::MAX_FLUX_INVENTORY_ENTRIES)
                    .map(|index| {
                        if index == helpers::MAX_FLUX_INVENTORY_ENTRIES - 2 {
                            json!({
                            "id":"tenant-46_widget_resource_group.example.io_Widget",
                            "version":"v2"
                            })
                        } else if index == helpers::MAX_FLUX_INVENTORY_ENTRIES - 1 {
                            json!({"id":"tenant-47_alias_widgets.example.io_Widget","v":"v3"})
                        } else {
                            let namespace = index % 100;
                            let name = if index.is_multiple_of(2) {
                                format!("ledger-{index}")
                            } else {
                                format!("checkout-{index}-pod")
                            };
                            json!({"id":format!("tenant-{namespace}_{name}__Pod"),"version":"v1"})
                        }
                    })
                    .collect::<Vec<_>>()
            } else if kind == "HelmRelease" && owner_index == 0 {
                vec![json!({"id":"tenant-0_ledger-0__Pod","version":"v1"})]
            } else if kind == "Kustomization" && owner_index == 1 {
                vec![
                    json!({"id":"tenant-0__example.io_Widget","version":"v1"}),
                    json!({"id":"tenant-1_missing-version_example.io_Widget"}),
                ]
            } else {
                Vec::new()
            };
            let value = json!({
                "apiVersion":format!("{group}/v1"),"kind":kind,
                "metadata":{"name":format!("owner-{kind}-{owner_index}"),"namespace":"flux-system"},
                "status":{"inventory":{"entries":entries}}
            });
            owners.push(serde_json::from_value(value).expect("Flux owner fixture"));
        }
        assert_eq!(owners.len(), helpers::MAX_FLUX_OWNER_OBJECTS);
        batches.push((kind.to_string(), owners));
    }
    FluxIndexFixture { batches }
}

pub(crate) fn build_flux_index_inner(fixture: FluxIndexFixture) -> helpers::FluxOwnershipIndex {
    helpers::flux_ownership_index_from_fetched_objects(fixture.batches)
}

pub fn build_flux_index_fixture(fixture: FluxIndexFixture) -> FluxIndexProjection {
    let index = build_flux_index_inner(fixture);
    FluxIndexProjection {
        retained_items: index.retained_item_count(),
        partial: index.partial,
    }
}

impl FluxIndexFixture {
    pub fn expected_owner_lookups(&self) -> [Option<(String, String, String)>; 5] {
        let index = build_flux_index_inner(self.clone());
        [
            inventory_owner(&index, "Pod", "v1", Some("tenant-0"), "ledger-0"),
            inventory_owner(
                &index,
                "Widget",
                "resource_group.example.io/v2",
                Some("tenant-46"),
                "widget",
            ),
            inventory_owner(
                &index,
                "Widget",
                "widgets.example.io/v3",
                Some("tenant-47"),
                "alias",
            ),
            inventory_owner(&index, "Widget", "example.io/v1", Some("tenant-0"), ""),
            inventory_owner(
                &index,
                "Widget",
                "example.io/v1",
                Some("tenant-1"),
                "missing-version",
            ),
        ]
    }
}

fn inventory_owner(
    index: &helpers::FluxOwnershipIndex,
    kind: &str,
    api_version: &str,
    namespace: Option<&str>,
    name: &str,
) -> Option<(String, String, String)> {
    let mut summary: ResourceSummary = serde_json::from_value(json!({
        "kind":kind,
        "cluster":"catalog-cluster",
        "name":name,
        "namespace":namespace,
        "age":"1m",
        "health":"unknown"
    }))
    .expect("inventory target summary");
    summary.api_version = Some(api_version.to_string());
    helpers::enrich_resource_summaries_with_flux_inventory(
        std::slice::from_mut(&mut summary),
        index,
    );
    summary
        .git_ops_owner
        .map(|owner| (owner.provider, owner.kind, owner.name))
}

pub fn resource_scope_fixture() -> ResourceScopeFixture {
    let mut summary_fixture = summaries::resource_summary_fixture(10_000);
    if let Some(pod) = summary_fixture
        .pods
        .iter_mut()
        .find(|pod| pod.metadata.name.as_deref() == Some("ledger-0"))
    {
        if let Some(annotations) = pod.metadata.annotations.as_mut() {
            annotations.remove("argocd.argoproj.io/name");
            annotations.remove("argocd.argoproj.io/tracking-id");
        }
    }
    if let Some(pod) = summary_fixture
        .pods
        .iter_mut()
        .find(|pod| pod.metadata.name.as_deref() == Some("ledger-100"))
    {
        if let Some(labels) = pod.metadata.labels.as_mut() {
            labels.remove("kustomize.toolkit.fluxcd.io/name");
            labels.remove("kustomize.toolkit.fluxcd.io/namespace");
        }
        if let Some(annotations) = pod.metadata.annotations.as_mut() {
            annotations.remove("argocd.argoproj.io/name");
            annotations.remove("argocd.argoproj.io/tracking-id");
        }
    }
    let now = fixed_now();
    let typed_rows = summary_fixture
        .pods
        .iter()
        .filter(|pod| {
            pod.metadata
                .namespace
                .as_deref()
                .and_then(|namespace| namespace.strip_prefix("tenant-"))
                .and_then(|namespace| namespace.parse::<usize>().ok())
                .is_some_and(|namespace| namespace < 10)
        })
        .map(|pod| crate::commands::resources::pod_resource_summary_at("catalog-cluster", pod, now))
        .collect::<Vec<_>>();
    let ownerless_target = typed_rows
        .iter()
        .find(|row| row.name == "ledger-100" && row.namespace.as_deref() == Some("tenant-0"))
        .expect("ownerless inventory target fixture");
    assert!(ownerless_target.argo_app.is_none());
    assert!(ownerless_target.git_ops_owner.is_none());
    let mut dynamic_fixture = summaries::dynamic_summary_fixture(10_000);
    dynamic_fixture.objects.retain(|object| {
        object
            .metadata
            .namespace
            .as_deref()
            .and_then(|namespace| namespace.strip_prefix("tenant-"))
            .and_then(|namespace| namespace.parse::<usize>().ok())
            .is_some_and(|namespace| namespace < 10)
    });
    let dynamic_rows = summaries::dynamic_summaries_at(&dynamic_fixture, fixed_now());
    let mut requests = (0..10)
        .map(|namespace| ResourceListRequest {
            kind: Some("Pod".to_string()),
            resource_kind: None,
            namespace: Some(format!("tenant-{namespace}")),
        })
        .collect::<Vec<_>>();
    requests.extend((0..10).map(|namespace| ResourceListRequest {
        kind: None,
        resource_kind: Some(DiscoveredResourceKind {
            group: dynamic_fixture.kind.group.clone(),
            version: dynamic_fixture.kind.version.clone(),
            api_version: dynamic_fixture.kind.api_version.clone(),
            kind: dynamic_fixture.kind.kind.clone(),
            plural: dynamic_fixture.kind.plural.clone(),
            short_names: dynamic_fixture.kind.short_names.clone(),
            namespaced: true,
        }),
        namespace: Some(format!("tenant-{namespace}")),
    }));
    requests.push(requests[0].clone());

    let mut primary = typed_rows;
    primary.extend(dynamic_rows);
    let mut duplicates = primary.iter().step_by(10).cloned().collect::<Vec<_>>();
    if let Some(replacement) = duplicates.first_mut() {
        replacement.status = Some("duplicate-last-row-marker".to_string());
    }
    ResourceScopeFixture {
        requests,
        batches: vec![primary, duplicates],
        index: build_flux_index_inner(flux_index_fixture()),
    }
}

pub fn scope_fetch_plan(
    requests: Vec<ResourceListRequest>,
) -> Result<Vec<(String, Option<String>)>, AppError> {
    resources::resource_scope_fetch_plan(requests)
}

pub fn assemble_resource_scope(
    fixture: ResourceScopeFixture,
) -> Result<Vec<ResourceSummary>, AppError> {
    let ResourceScopeFixture {
        requests,
        batches,
        index,
    } = fixture;
    resources::resource_scope_post_fetch(requests, batches, index)
}
