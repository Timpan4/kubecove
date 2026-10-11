use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::core::{ApiResource, DynamicObject};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    commands::argo::{
        benchmark_app_project_summary_from_object, benchmark_application_set_summary_from_object,
        benchmark_application_summary_from_object, benchmark_connected_inspector,
        benchmark_connected_selected_comparison, benchmark_kubernetes_inspector,
        benchmark_kubernetes_selected_comparison, benchmark_verify_kubernetes_application_identity,
    },
    models::{
        AppError, ArgoAppProjectSummary, ArgoApplicationInspector, ArgoApplicationRef,
        ArgoApplicationSetSummary, ArgoApplicationSummary, ArgoManagedResource,
        ArgoResourceComparison,
    },
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgoListProjectionBatch {
    pub applications: Vec<ArgoApplicationSummary>,
    pub application_sets: Vec<ArgoApplicationSetSummary>,
    pub projects: Vec<ArgoAppProjectSummary>,
}

pub fn list_objects() -> (Vec<DynamicObject>, Vec<DynamicObject>, Vec<DynamicObject>) {
    let applications = (0..250).map(application_object).collect::<Vec<_>>();
    let appsets = (0..250).map(application_set_object).collect::<Vec<_>>();
    let projects = (0..250).map(app_project_object).collect::<Vec<_>>();
    (applications, appsets, projects)
}

pub fn project_list_summaries(
    applications: &[DynamicObject],
    appsets: &[DynamicObject],
    projects: &[DynamicObject],
) -> ArgoListProjectionBatch {
    ArgoListProjectionBatch {
        applications: applications
            .iter()
            .filter_map(|object| benchmark_application_summary_from_object("benchmark", object))
            .collect(),
        application_sets: appsets
            .iter()
            .filter_map(|object| benchmark_application_set_summary_from_object("benchmark", object))
            .collect(),
        projects: projects
            .iter()
            .filter_map(|object| benchmark_app_project_summary_from_object("benchmark", object))
            .collect(),
    }
}

pub fn kubernetes_inspector_input() -> DynamicObject {
    application_object_with_status(17, 250)
}

pub fn connected_inspector_inputs() -> (ArgoApplicationRef, Value, Value) {
    let name = "checkout-17";
    let mut managed_items = (0..250).map(managed_item).collect::<Vec<_>>();
    managed_items[0] = secret_managed_item("late-secret");
    managed_items[0]["group"] = json!("example.io");
    managed_items[1] = secret_managed_item("late-secret");
    managed_items[1]["kind"] = json!("ConfigMap");
    // A same-name non-Secret must carry its own document, not Secret test data.
    managed_items[1]["targetState"] = json!({
        "apiVersion":"v1", "kind":"ConfigMap",
        "metadata":{"name":"late-secret","namespace":"payments"},
        "data":{"config":"enabled"}
    });
    managed_items[1]["liveState"] = managed_items[1]["targetState"].clone();
    managed_items[1]["normalizedLiveState"] = managed_items[1]["targetState"].clone();
    managed_items[1]["predictedLiveState"] = Value::Null;
    managed_items[2] = secret_managed_item("late-secret");
    managed_items[2]["namespace"] = json!("other-namespace");
    managed_items[249] = secret_managed_item("late-secret");
    let mut response_status = status_value(250);
    // Status redaction traverses structured objects. Managed state strings
    // belong in the managed-resources response, where production decodes them.
    response_status["embeddedSecret"] = secret_managed_item("status-secret")["liveState"].clone();
    let response = json!({
        "metadata": {
            "name": name,
            "namespace": "argocd",
            "resourceVersion": "318",
            "uid": "argo-uid-17",
            "annotations": {"argocd.argoproj.io/refresh":"normal"}
        },
        "apiVersion": "argoproj.io/v1alpha1",
        "operation": {"sync": {"revision": "main"}},
        "status": response_status
    });
    let managed = json!({"items":managed_items});
    (application_ref(name), response, managed)
}

pub fn selected_kubernetes_comparison(
    application: &DynamicObject,
    target: &ArgoManagedResource,
) -> Result<ArgoResourceComparison, AppError> {
    benchmark_kubernetes_selected_comparison(application, target)
}

pub fn selected_connected_comparison(
    managed: Value,
    target: &ArgoManagedResource,
) -> Result<ArgoResourceComparison, AppError> {
    benchmark_connected_selected_comparison(managed, target)
}

pub fn connected_inspector(
    application: ArgoApplicationRef,
    response: Value,
    managed: Value,
) -> ArgoApplicationInspector {
    benchmark_connected_inspector(application, response, managed)
}

pub fn kubernetes_inspector(
    object: DynamicObject,
    application: &ArgoApplicationRef,
) -> Result<ArgoApplicationInspector, AppError> {
    benchmark_kubernetes_inspector(object, "benchmark", application)
}

pub fn identity_revalidation(
    current: &DynamicObject,
    expected: &ArgoApplicationRef,
) -> Result<(), AppError> {
    benchmark_verify_kubernetes_application_identity(current, expected)
}

pub fn application_ref(name: &str) -> ArgoApplicationRef {
    ArgoApplicationRef {
        name: name.to_string(),
        namespace: Some("argocd".to_string()),
        project: Some("payments".to_string()),
        uid: Some("argo-uid-17".to_string()),
        resource_version: Some("318".to_string()),
        context: Some("benchmark".to_string()),
        workspace_id: Some("workspace-17".to_string()),
        api_version: Some("argoproj.io/v1alpha1".to_string()),
    }
}

pub fn selected_resource(name: &str, kind: &str) -> ArgoManagedResource {
    selected_resource_in_namespace(name, kind, "payments")
}

pub fn selected_resource_in_namespace(
    name: &str,
    kind: &str,
    namespace: &str,
) -> ArgoManagedResource {
    ArgoManagedResource {
        group: Some(if kind == "Secret" {
            String::new()
        } else {
            "apps".to_string()
        }),
        version: Some("v1".to_string()),
        kind: Some(kind.to_string()),
        namespace: Some(namespace.to_string()),
        name: Some(name.to_string()),
        status: Some("Synced".to_string()),
        health: Some("Healthy".to_string()),
        hook: Some(false),
        requires_pruning: Some(false),
        target_state: None,
        live_state: None,
    }
}

pub fn encode<T: serde::Serialize>(value: &T) -> String {
    super::encode_complete_response(value)
}

fn application_object(index: usize) -> DynamicObject {
    application_object_with_status(index, 4)
}

fn application_object_with_status(index: usize, item_count: usize) -> DynamicObject {
    let sources = (0..4)
        .map(|source| {
            json!({
                "repoURL": format!("https://git.example/repository-{source}"),
                "targetRevision": if source == 0 && index == 249 {
                    Value::Null
                } else {
                    Value::String((if source == 0 { "main" } else { "release" }).to_string())
                },
                "path": format!("charts/service-{source}"),
                "ref": format!("source-{source}")
            })
        })
        .collect::<Vec<_>>();
    let mut status = status_value(item_count);
    if item_count > 249 {
        let resources = status["resources"]
            .as_array_mut()
            .expect("inspector resources fixture");
        resources[0]["name"] = json!("deployment-249");
        resources[0]["group"] = json!("example.io");
        resources[1]["name"] = json!("deployment-249");
        resources[1]["kind"] = json!("StatefulSet");
        resources[2]["name"] = json!("deployment-249");
        resources[2]["namespace"] = json!("other-namespace");
    }
    let data = json!({
        "spec": {
            "project": "payments",
            "destination": {"namespace":"payments", "server":"https://kubernetes.default"},
            "sources": sources
        },
        "operation": {"sync": {"revision":"main"}},
        "status": status
    });
    let name = format!("checkout-{index}");
    let mut object = DynamicObject::new(&name, &api_resource("Application", "applications"))
        .within("argocd")
        .data(data);
    object.metadata = ObjectMeta {
        name: Some(name),
        namespace: Some("argocd".to_string()),
        uid: Some(format!("argo-uid-{index}")),
        resource_version: Some("318".to_string()),
        annotations: Some(std::collections::BTreeMap::from([(
            "argocd.argoproj.io/refresh".to_string(),
            "normal".to_string(),
        )])),
        ..Default::default()
    };
    object
}

fn application_set_object(index: usize) -> DynamicObject {
    let object = json!({
        "spec": {
            "project":"payments",
            "generatorParams":[{"dest-namespace":"tenant-a", "dest-server":"in-cluster"}],
            "source":{"repoURL":"https://git.example/payments", "targetRevision":"main"}
        },
        "status": {
            "health":{"status":"Healthy"},
            "conditions":[{"type":"ResourcesUpToDate", "status":"True"}]
        }
    });
    let name = format!("tenant-apps-{index}");
    DynamicObject::new(&name, &api_resource("ApplicationSet", "applicationsets"))
        .within("argocd")
        .data(object)
}

fn app_project_object(index: usize) -> DynamicObject {
    let name = format!("project-{index}");
    DynamicObject::new(&name, &api_resource("AppProject", "appprojects"))
        .within("argocd")
        .data(json!({
            "spec":{"description":"Payment workloads with namespace and repository policies"},
            "status":{"conditions":[{"type":"ValidSpec", "message":"project is valid"}]}
        }))
}

fn api_resource(kind: &str, plural: &str) -> ApiResource {
    ApiResource {
        group: "argoproj.io".to_string(),
        version: "v1alpha1".to_string(),
        api_version: "argoproj.io/v1alpha1".to_string(),
        kind: kind.to_string(),
        plural: plural.to_string(),
    }
}

fn status_value(count: usize) -> Value {
    json!({
        "health":{"status":"Healthy"},
            "sync":{"status":"Synced", "revision":"main", "revisions":["main", "release", "main", "release"]},
        "operationState":{"phase":"Succeeded", "message":"sync complete"},
        "history":(0..count).map(|index| json!({
            "id": index,
            "revision": format!("revision-{index}"),
            "revisions": [format!("revision-{index}"), "base"],
            "deployedAt": "2026-10-10T00:00:00Z",
            "initiatedBy": {"username":"release-bot"},
            "source": {"repoURL":"https://git.example/payments"},
            "sources": [{"repoURL":"https://git.example/payments"}]
        })).collect::<Vec<_>>(),
        "resources":(0..count).map(|index| json!({
            "group":"apps", "version":"v1", "kind":"Deployment",
            "namespace":format!("tenant-{}", index % 25),
            "name":format!("deployment-{index:03}"), "status":"Synced",
            "health":{"status":"Healthy"}, "hook":false, "requiresPruning":false
        })).collect::<Vec<_>>(),
        "conditions":(0..count).map(|index| json!({
            "type":"ComparisonError", "message":format!("condition-{index}"), "lastTransitionTime":"2026-10-10T00:00:00Z"
        })).collect::<Vec<_>>()
    })
}

fn managed_item(index: usize) -> Value {
    json!({
        "group":"apps", "version":"v1", "kind":"Deployment",
        "namespace":format!("tenant-{}", index % 25),
        "name":if index == 249 { "deployment-249".to_string() } else { format!("deployment-{index:03}") },
        "status":"Synced", "health":{"status":"Healthy"}, "modified":index.is_multiple_of(2),
        "targetState": {"apiVersion":"apps/v1", "kind":"Deployment", "metadata":{"name":format!("deployment-{index:03}")}, "spec":{"replicas":3}},
        "liveState": "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: deployment\nspec:\n  replicas: 3",
        "normalizedLiveState": "not-json: [",
        "predictedLiveState": Value::Null
    })
}

fn secret_managed_item(name: &str) -> Value {
    let target_state = json!({
        "kind":"Secret",
        "data":{"password":"target-secret"},
        "stringData":{"token":"target-token"}
    })
    .to_string();
    json!({
        "group":"", "version":"v1", "kind":"Secret", "namespace":"payments", "name":name,
        "status":"Synced", "modified":true,
        "targetState":target_state,
        "liveState":{"apiVersion":"v1", "kind":"Secret", "metadata":{"name":name, "annotations":{"kubectl.kubernetes.io/last-applied-configuration":"secret-config"}}, "data":{"password":"live-secret"}},
        "normalizedLiveState":"kind: Secret\ndata:\n  password: normalized-secret",
        "predictedLiveState":"Secret: malformed secret state"
    })
}
