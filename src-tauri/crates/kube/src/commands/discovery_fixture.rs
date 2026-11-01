//! Test API server for discovery-backed reads. Besides the given kinds, it
//! serves an aggregated API that publishes `"verbs": null`, as `KubeVirt`'s
//! `subresources.kubevirt.io` does on real clusters. kube's aggregated
//! discovery parser rejects that `null`, so a read that walks every group
//! fails on such clusters.

use kube::client::Body;
use kube::Client;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::convert::Infallible;

const NULL_VERBS_GROUP: &str = "subresources.kubevirt.io";
const NULL_VERBS_VERSION: &str = "v1";

#[derive(Clone, Copy)]
pub struct FixtureKind {
    /// Empty for the core group.
    pub group: &'static str,
    pub version: &'static str,
    pub kind: &'static str,
    pub plural: &'static str,
    pub namespaced: bool,
}

/// A client whose discovery requests return `kinds` plus the null-verbs
/// aggregated API, in both aggregated and per-group form.
pub fn client(kinds: &[FixtureKind]) -> Client {
    serve(kinds, &[], true)
}

/// Like [`client`], but the per-group list also names each `(group, version)`
/// in `unavailable`, ahead of the group's served versions so it is the
/// preferred version, and requests for those answer 503, as kube-apiserver
/// does when an aggregated API server is down.
pub fn client_with_unavailable_groups(
    kinds: &[FixtureKind],
    unavailable: &[(&'static str, &'static str)],
) -> Client {
    serve(kinds, unavailable, true)
}

/// Like [`client`], but the group lists at `/api` and `/apis` answer 503, as
/// an overloaded or restarting API server does.
pub fn client_with_unavailable_group_list(kinds: &[FixtureKind]) -> Client {
    serve(kinds, &[], false)
}

fn serve(
    kinds: &[FixtureKind],
    unavailable: &[(&'static str, &'static str)],
    group_list_available: bool,
) -> Client {
    let kinds = kinds.to_vec();
    let unavailable = unavailable.to_vec();
    let service = tower::service_fn(move |request: http::Request<Body>| {
        let aggregated = request
            .headers()
            .get(http::header::ACCEPT)
            .and_then(|accept| accept.to_str().ok())
            .is_some_and(|accept| accept.contains("apidiscovery.k8s.io"));
        let path = request.uri().path();
        let (status, body) = if !group_list_available && matches!(path, "/api" | "/apis") {
            service_unavailable(path)
        } else {
            route(path, aggregated, &kinds, &unavailable)
        };
        let response = http::Response::builder()
            .status(status)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&body).expect("fixture JSON")))
            .expect("fixture response");
        std::future::ready(Ok::<_, Infallible>(response))
    });
    Client::new(service, "default")
}

fn route(
    path: &str,
    aggregated: bool,
    kinds: &[FixtureKind],
    unavailable: &[(&'static str, &'static str)],
) -> (u16, Value) {
    match (path, aggregated) {
        ("/apis", true) => (200, aggregated_list(kinds, false)),
        ("/api", true) => (200, aggregated_list(kinds, true)),
        ("/apis", false) => (200, group_list(kinds, unavailable)),
        ("/api", false) => (200, json!({ "kind": "APIVersions", "versions": ["v1"] })),
        ("/api/v1", false) => (200, resource_list("v1", kinds_in(kinds, "", "v1"))),
        (path, false) => match path
            .strip_prefix("/apis/")
            .and_then(|gv| gv.split_once('/'))
        {
            Some((NULL_VERBS_GROUP, NULL_VERBS_VERSION)) => (200, null_verbs_resource_list()),
            Some(group_version) if unavailable.contains(&group_version) => {
                service_unavailable(path)
            }
            Some((group, version)) if !kinds_in(kinds, group, version).is_empty() => (
                200,
                resource_list(
                    &format!("{group}/{version}"),
                    kinds_in(kinds, group, version),
                ),
            ),
            _ => not_found(path),
        },
        (path, true) => not_found(path),
    }
}

fn kinds_in<'a>(kinds: &'a [FixtureKind], group: &str, version: &str) -> Vec<&'a FixtureKind> {
    kinds
        .iter()
        .filter(|kind| kind.group == group && kind.version == version)
        .collect()
}

fn group_versions(kinds: &[FixtureKind], core: bool) -> BTreeMap<&'static str, Vec<&'static str>> {
    let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for kind in kinds.iter().filter(|kind| kind.group.is_empty() == core) {
        let versions = groups.entry(kind.group).or_default();
        if !versions.contains(&kind.version) {
            versions.push(kind.version);
        }
    }
    groups
}

fn aggregated_list(kinds: &[FixtureKind], core: bool) -> Value {
    let mut items: Vec<Value> = group_versions(kinds, core)
        .into_iter()
        .map(|(group, versions)| {
            let versions: Vec<Value> = versions
                .into_iter()
                .map(|version| {
                    let resources: Vec<Value> = kinds_in(kinds, group, version)
                        .into_iter()
                        .map(|kind| {
                            json!({
                                "resource": kind.plural,
                                "responseKind": { "group": group, "version": version, "kind": kind.kind },
                                "scope": if kind.namespaced { "Namespaced" } else { "Cluster" },
                                "singularResource": "",
                                "verbs": ["get", "list", "watch"],
                            })
                        })
                        .collect();
                    json!({ "version": version, "resources": resources, "freshness": "Current" })
                })
                .collect();
            json!({ "metadata": { "name": group }, "versions": versions })
        })
        .collect();
    if !core {
        items.push(json!({
            "metadata": { "name": NULL_VERBS_GROUP },
            "versions": [{
                "version": NULL_VERBS_VERSION,
                "resources": [{
                    "resource": "expand-vm-spec",
                    "responseKind": { "group": "", "version": "", "kind": "" },
                    "scope": "Namespaced",
                    "singularResource": "",
                    "verbs": null,
                }],
                "freshness": "Current",
            }],
        }));
    }
    json!({
        "kind": "APIGroupDiscoveryList",
        "apiVersion": "apidiscovery.k8s.io/v2",
        "metadata": {},
        "items": items,
    })
}

fn group_list(kinds: &[FixtureKind], unavailable: &[(&'static str, &'static str)]) -> Value {
    let mut versions_by_group = group_versions(kinds, false);
    versions_by_group.insert(NULL_VERBS_GROUP, vec![NULL_VERBS_VERSION]);
    for (group, version) in unavailable {
        let versions = versions_by_group.entry(group).or_default();
        if !versions.contains(version) {
            versions.insert(0, version);
        }
    }
    let groups: Vec<Value> = versions_by_group
        .into_iter()
        .map(|(group, versions)| api_group(group, &versions))
        .collect();
    json!({ "kind": "APIGroupList", "apiVersion": "v1", "groups": groups })
}

fn api_group(group: &str, versions: &[&str]) -> Value {
    let versions: Vec<Value> = versions
        .iter()
        .map(|version| json!({ "groupVersion": format!("{group}/{version}"), "version": version }))
        .collect();
    json!({ "name": group, "versions": versions, "preferredVersion": versions[0] })
}

fn resource_list(group_version: &str, kinds: Vec<&FixtureKind>) -> Value {
    let resources: Vec<Value> = kinds
        .into_iter()
        .map(|kind| {
            json!({
                "name": kind.plural,
                "singularName": "",
                "namespaced": kind.namespaced,
                "kind": kind.kind,
                "verbs": ["get", "list", "watch"],
            })
        })
        .collect();
    json!({
        "kind": "APIResourceList",
        "apiVersion": "v1",
        "groupVersion": group_version,
        "resources": resources,
    })
}

fn null_verbs_resource_list() -> Value {
    json!({
        "kind": "APIResourceList",
        "apiVersion": "v1",
        "groupVersion": format!("{NULL_VERBS_GROUP}/{NULL_VERBS_VERSION}"),
        "resources": [{
            "name": "expand-vm-spec",
            "singularName": "",
            "namespaced": true,
            "kind": "",
            "verbs": null,
        }],
    })
}

fn service_unavailable(path: &str) -> (u16, Value) {
    (
        503,
        json!({
            "kind": "Status", "apiVersion": "v1", "metadata": {}, "status": "Failure",
            "message": format!("{path}: the server is currently unable to handle the request"),
            "reason": "ServiceUnavailable", "code": 503,
        }),
    )
}

fn not_found(path: &str) -> (u16, Value) {
    (
        404,
        json!({
            "kind": "Status", "apiVersion": "v1", "metadata": {}, "status": "Failure",
            "message": format!("{path} not found"), "reason": "NotFound", "code": 404,
        }),
    )
}
