//! The controlling owner a delete review shows, and whether that owner
//! recreates the deleted object. Only built-in controllers count: a custom
//! operator may or may not replace what it owns, so its objects keep the
//! typed-name confirmation (ADR 0024).

use crate::models::ResourceActionController;
use serde_json::Value;

/// Built-in controllers that replace a deleted object, by target kind:
/// (target kind, owner API group, owner kind). The core group is "".
const RECREATING_OWNERS: &[(&str, &str, &str)] = &[
    ("Pod", "apps", "ReplicaSet"),
    ("Pod", "apps", "StatefulSet"),
    ("Pod", "apps", "DaemonSet"),
    ("Pod", "batch", "Job"),
    ("Pod", "", "ReplicationController"),
    ("ReplicaSet", "apps", "Deployment"),
];

/// The owner reference marked `controller: true` on `object`.
pub(super) fn action_controller(
    target_kind: &str,
    object: &Value,
) -> Option<ResourceActionController> {
    let owner = object
        .pointer("/metadata/ownerReferences")?
        .as_array()?
        .iter()
        .find(|owner| owner.get("controller").and_then(Value::as_bool) == Some(true))?;
    let kind = owner.get("kind")?.as_str()?;
    let name = owner.get("name")?.as_str()?;
    let api_version = owner.get("apiVersion")?.as_str()?;
    let group = api_version
        .rsplit_once('/')
        .map_or("", |(group, _version)| group);
    // A finished Pod keeps its owner reference, but its controller has already
    // counted it as done or replaced it, so deleting it creates nothing new.
    let finished = matches!(
        object.pointer("/status/phase").and_then(Value::as_str),
        Some("Succeeded" | "Failed")
    );
    let recreates = !finished
        && RECREATING_OWNERS
            .iter()
            .any(|&(target, owner_group, owner_kind)| {
                target == target_kind && owner_group == group && owner_kind == kind
            });
    Some(ResourceActionController {
        kind: kind.to_owned(),
        name: name.to_owned(),
        recreates,
    })
}

#[cfg(test)]
mod tests {
    use super::action_controller;
    use serde_json::{Value, json};

    fn owned(owners: Value) -> Value {
        json!({ "metadata": { "name": "web-7d9c9b7f8d-k2r9p", "ownerReferences": owners } })
    }

    fn recreates(target_kind: &str, object: &Value) -> Option<bool> {
        action_controller(target_kind, object).map(|controller| controller.recreates)
    }

    // Failure modes, written before the code:
    // 1. an object with no owners must still require typing;
    // 2. an owner without `controller: true` does not replace the object;
    // 3. the controller may follow other owner references;
    // 4. a CRD that reuses a built-in kind name in another group is not trusted;
    // 5. a built-in controller only recreates the kind it manages;
    // 6. a malformed owner reference must not count as a controller;
    // 7. a finished Pod keeps its owner reference but is never replaced.
    #[test]
    fn only_built_in_controllers_of_the_target_kind_recreate() {
        assert_eq!(
            action_controller("Pod", &json!({ "metadata": { "name": "bare" } })),
            None
        );
        assert_eq!(
            action_controller(
                "Pod",
                &owned(
                    json!([{ "apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "web-7d9c9b7f8d" }])
                )
            ),
            None
        );
        let controller = action_controller(
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
        assert_eq!(recreates("ReplicaSet", &replica_set), Some(true));
        let malformed =
            owned(json!([{ "apiVersion": "apps/v1", "kind": "ReplicaSet", "controller": true }]));
        assert_eq!(action_controller("Pod", &malformed), None);
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
}
