//! The controlling owner a delete review shows, and whether that owner
//! recreates the deleted object. Only built-in controllers count: a custom
//! operator may or may not replace what it owns, so its objects keep the
//! typed-name confirmation (ADR 0024).

use crate::models::ResourceActionController;
use kube::api::{Api, ApiResource, DynamicObject};
use kube::core::GroupVersionKind;
use serde_json::Value;

/// Built-in controllers that replace a deleted object, by target kind:
/// (target kind, owner API group, owner kind). The core group is "".
/// A `Deployment` is left out for `ReplicaSet` targets: it recreates only the
/// `ReplicaSet` of its current template, so deleting an older one loses that revision.
const RECREATING_OWNERS: &[(&str, &str, &str)] = &[
    ("Pod", "apps", "ReplicaSet"),
    ("Pod", "apps", "StatefulSet"),
    ("Pod", "apps", "DaemonSet"),
    ("Pod", "batch", "Job"),
    ("Pod", "", "ReplicationController"),
];

/// A trusted built-in owner that the preview still has to fetch to confirm
/// it exists and can create a replacement.
pub(super) struct OwnerToConfirm {
    gvk: GroupVersionKind,
    name: String,
    uid: String,
}

/// The controlling owner for the review. `recreates` is true only when a
/// fetch of the live owner confirms it (`owner_can_replace`); any read error,
/// such as `NotFound` or `Forbidden`, keeps the typed-name confirmation.
pub(super) async fn confirmed_controller(
    client: kube::Client,
    namespace: &str,
    target_kind: &str,
    object: &Value,
) -> Option<ResourceActionController> {
    let (mut controller, owner) = action_controller(target_kind, object)?;
    if let Some(owner) = owner {
        let api: Api<DynamicObject> =
            Api::namespaced_with(client, namespace, &ApiResource::from_gvk(&owner.gvk));
        controller.recreates = match api.get(&owner.name).await {
            Ok(live) => serde_json::to_value(&live)
                .is_ok_and(|live| owner_can_replace(&live, &owner.uid, &owner.gvk.kind, object)),
            Err(_) => false,
        };
    }
    Some(controller)
}

/// The live owner is the one the reference names, is not being deleted (an
/// orphaning or foreground delete leaves the reference in place for a while),
/// and still desires replacement work for this target.
fn owner_can_replace(owner: &Value, uid: &str, kind: &str, target: &Value) -> bool {
    if owner.pointer("/metadata/uid").and_then(Value::as_str) != Some(uid)
        || owner.pointer("/metadata/deletionTimestamp").is_some()
    {
        return false;
    }
    match kind {
        "StatefulSet" => statefulset_desires_pod(owner, target),
        "Job" => job_has_replacement_work(owner),
        "ReplicaSet" | "ReplicationController" => {
            owner
                .pointer("/spec/replicas")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                > 0
        }
        "DaemonSet" => true,
        _ => false,
    }
}

fn statefulset_desires_pod(owner: &Value, target: &Value) -> bool {
    let Some(owner_name) = owner.pointer("/metadata/name").and_then(Value::as_str) else {
        return false;
    };
    let Some(suffix) = target
        .pointer("/metadata/name")
        .and_then(Value::as_str)
        .and_then(|name| name.strip_prefix(owner_name))
        .and_then(|suffix| suffix.strip_prefix('-'))
    else {
        return false;
    };
    let Ok(ordinal) = suffix.parse::<u64>() else {
        return false;
    };
    if ordinal.to_string() != suffix {
        return false;
    }
    // Kubernetes defaults StatefulSet start to zero and replicas to one.
    let start = owner
        .pointer("/spec/ordinals/start")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let replicas = owner
        .pointer("/spec/replicas")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    ordinal
        .checked_sub(start)
        .is_some_and(|index| index < replicas)
}

fn job_has_replacement_work(owner: &Value) -> bool {
    if owner.pointer("/spec/suspend").and_then(Value::as_bool) == Some(true)
        || owner.pointer("/spec/parallelism").and_then(Value::as_u64) == Some(0)
        || owner.pointer("/spec/managedBy").and_then(Value::as_str)
            .is_some_and(|controller| controller != "kubernetes.io/job-controller")
        // Indexed Pods can be obsolete even while other indexes need work.
        || owner.pointer("/spec/completionMode").and_then(Value::as_str) == Some("Indexed")
    {
        return false;
    }
    if owner
        .pointer("/status/conditions")
        .and_then(Value::as_array)
        .is_some_and(|conditions| {
            conditions.iter().any(|condition| {
                condition.get("status").and_then(Value::as_str) == Some("True")
                    && matches!(
                        condition.get("type").and_then(Value::as_str),
                        Some("FailureTarget" | "Failed" | "SuccessCriteriaMet" | "Complete")
                    )
            })
        })
    {
        return false;
    }
    let succeeded = owner
        .pointer("/status/succeeded")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    match owner.pointer("/spec/completions").and_then(Value::as_u64) {
        Some(completions) => succeeded < completions,
        // A work-queue Job stops creating Pods after its first success.
        None => succeeded == 0,
    }
}

/// The owner reference marked `controller: true` on `object`, and the owner
/// to fetch when its reference matches a recreating built-in controller.
fn action_controller(
    target_kind: &str,
    object: &Value,
) -> Option<(ResourceActionController, Option<OwnerToConfirm>)> {
    let owner = object
        .pointer("/metadata/ownerReferences")?
        .as_array()?
        .iter()
        .find(|owner| owner.get("controller").and_then(Value::as_bool) == Some(true))?;
    let kind = owner.get("kind")?.as_str()?;
    let name = owner.get("name")?.as_str()?;
    let api_version = owner.get("apiVersion")?.as_str()?;
    // `group/version`, or exactly `v1` for the core group; anything else matches no group.
    let group_version = match api_version.split_once('/') {
        Some((group, version))
            if !group.is_empty() && !version.is_empty() && !version.contains('/') =>
        {
            Some((group, version))
        }
        Some(_) => None,
        None => (api_version == "v1").then_some(("", "v1")),
    };
    let group = group_version.map(|(group, _)| group);
    // A finished Pod keeps its owner reference, but its controller has already
    // counted it as done or replaced it, so deleting it creates nothing new.
    let finished = matches!(
        object.pointer("/status/phase").and_then(Value::as_str),
        Some("Succeeded" | "Failed")
    );
    let trusted = !finished
        && RECREATING_OWNERS
            .iter()
            .any(|&(target, owner_group, owner_kind)| {
                target == target_kind && Some(owner_group) == group && owner_kind == kind
            });
    let uid = owner
        .get("uid")
        .and_then(Value::as_str)
        .filter(|uid| !uid.is_empty());
    let to_confirm = match (trusted, group_version, uid) {
        (true, Some((group, version)), Some(uid)) => Some(OwnerToConfirm {
            gvk: GroupVersionKind::gvk(group, version, kind),
            name: name.to_owned(),
            uid: uid.to_owned(),
        }),
        _ => None,
    };
    Some((
        ResourceActionController {
            kind: kind.to_owned(),
            name: name.to_owned(),
            recreates: to_confirm.is_some(),
        },
        to_confirm,
    ))
}

#[cfg(test)]
mod tests {
    use super::{action_controller, owner_can_replace};
    use serde_json::{Value, json};

    /// Owner references as the API server returns them, with a `uid`.
    fn owned(mut owners: Value) -> Value {
        for owner in owners.as_array_mut().expect("owner list") {
            owner["uid"] = json!("owner-uid");
        }
        json!({ "metadata": { "name": "web-7d9c9b7f8d-k2r9p", "ownerReferences": owners } })
    }

    fn recreates(target_kind: &str, object: &Value) -> Option<bool> {
        action_controller(target_kind, object).map(|(controller, _)| controller.recreates)
    }

    // Failure modes, written before the code:
    // 1. an object with no owners must still require typing;
    // 2. an owner without `controller: true` does not replace the object;
    // 3. the controller may follow other owner references;
    // 4. a CRD that reuses a built-in kind name in another group is not trusted;
    // 5. a built-in controller only recreates the kind it manages;
    // 6. a malformed owner reference must not count as a controller;
    // 7. a finished Pod keeps its owner reference but is never replaced;
    // 8. an apiVersion other than `group/version` or `v1` names no trusted group;
    // 9. a Deployment does not recreate an older ReplicaSet revision;
    // 10. an owner reference without a `uid` cannot be matched to the live owner.
    #[test]
    fn only_built_in_controllers_of_the_target_kind_recreate() {
        assert!(action_controller("Pod", &json!({ "metadata": { "name": "bare" } })).is_none());
        assert!(
            action_controller(
                "Pod",
                &owned(
                    json!([{ "apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "web-7d9c9b7f8d" }])
                )
            )
            .is_none()
        );
        let no_uid = json!({ "metadata": { "ownerReferences": [
            { "apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "web", "controller": true }
        ] } });
        assert_eq!(recreates("Pod", &no_uid), Some(false));
        let (controller, _) = action_controller(
            "Pod",
            &owned(json!([
                { "apiVersion": "v1", "kind": "ConfigMap", "name": "ref" },
                { "apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "web-7d9c9b7f8d", "controller": true }
            ])),
        )
        .expect("controller owner");
        assert_eq!(
            (
                controller.kind.as_str(),
                controller.name.as_str(),
                controller.recreates
            ),
            ("ReplicaSet", "web-7d9c9b7f8d", true)
        );
        for (api_version, kind) in [
            ("apps/v1", "StatefulSet"),
            ("apps/v1", "DaemonSet"),
            ("batch/v1", "Job"),
            ("v1", "ReplicationController"),
        ] {
            let object = owned(
                json!([{ "apiVersion": api_version, "kind": kind, "name": "owner", "controller": true }]),
            );
            assert_eq!(recreates("Pod", &object), Some(true), "{kind}");
        }
        let custom = owned(
            json!([{ "apiVersion": "example.com/v1", "kind": "ReplicaSet", "name": "x", "controller": true }]),
        );
        assert_eq!(recreates("Pod", &custom), Some(false));
        let operator = owned(
            json!([{ "apiVersion": "argoproj.io/v1alpha1", "kind": "Rollout", "name": "x", "controller": true }]),
        );
        assert_eq!(recreates("Pod", &operator), Some(false));
        let wrong_target = owned(
            json!([{ "apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "x", "controller": true }]),
        );
        assert_eq!(recreates("ConfigMap", &wrong_target), Some(false));
        let replica_set = owned(
            json!([{ "apiVersion": "apps/v1", "kind": "Deployment", "name": "web", "controller": true }]),
        );
        assert_eq!(recreates("ReplicaSet", &replica_set), Some(false));
        let unversioned = owned(
            json!([{ "apiVersion": "bogus", "kind": "ReplicationController", "name": "x", "controller": true }]),
        );
        assert_eq!(recreates("Pod", &unversioned), Some(false));
        for (api_version, kind) in [
            ("/v1", "ReplicationController"),
            ("apps/", "ReplicaSet"),
            ("apps/v1/extra", "ReplicaSet"),
        ] {
            let object = owned(
                json!([{ "apiVersion": api_version, "kind": kind, "name": "x", "controller": true }]),
            );
            assert_eq!(recreates("Pod", &object), Some(false), "{api_version}");
        }
        let malformed =
            owned(json!([{ "apiVersion": "apps/v1", "kind": "ReplicaSet", "controller": true }]));
        assert!(action_controller("Pod", &malformed).is_none());
        for (api_version, kind, phase) in [
            ("batch/v1", "Job", "Succeeded"),
            ("apps/v1", "ReplicaSet", "Failed"),
        ] {
            let mut object = owned(
                json!([{ "apiVersion": api_version, "kind": kind, "name": "owner", "controller": true }]),
            );
            object["status"] = json!({ "phase": phase });
            assert_eq!(recreates("Pod", &object), Some(false), "{kind} {phase}");
        }
        let mut running = owned(
            json!([{ "apiVersion": "batch/v1", "kind": "Job", "name": "owner", "controller": true }]),
        );
        running["status"] = json!({ "phase": "Running" });
        assert_eq!(recreates("Pod", &running), Some(true));
    }

    // Failure modes of the live owner, written before the code:
    // 1. a same-named owner created after the original has another uid;
    // 2. an owner scaled to zero replaces nothing;
    // 3. a suspended Job replaces nothing.
    // A terminating, missing or unreadable owner is covered at the preview seam
    // in `resource_actions_tests.rs`.
    #[test]
    fn only_a_live_active_owner_with_the_referenced_uid_replaces() {
        let live = json!({ "metadata": { "uid": "owner-uid" }, "spec": { "replicas": 2 } });
        assert!(owner_can_replace(
            &live,
            "owner-uid",
            "ReplicaSet",
            &json!({})
        ));
        assert!(owner_can_replace(
            &json!({ "metadata": { "uid": "owner-uid" } }),
            "owner-uid",
            "ReplicaSet",
            &json!({})
        ));
        assert!(!owner_can_replace(
            &live,
            "other-uid",
            "ReplicaSet",
            &json!({})
        ));
        let mut scaled_down = live.clone();
        scaled_down["spec"]["replicas"] = json!(0);
        assert!(!owner_can_replace(
            &scaled_down,
            "owner-uid",
            "ReplicaSet",
            &json!({})
        ));
    }
}
