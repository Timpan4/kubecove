use kube::api::ApiResource;

/// A built-in Kubernetes kind read through typed clients.
pub(crate) struct BuiltinKind {
    pub(crate) kind: &'static str,
    pub(crate) group: &'static str,
    pub(crate) version: &'static str,
    pub(crate) plural: &'static str,
    pub(crate) namespaced: bool,
    /// Listed by `list_resources` and served by details and YAML reads.
    pub(crate) table: bool,
    /// Accepted by selected-resource YAML apply (ADR 0006).
    pub(crate) apply: bool,
}

const fn entry(
    kind: &'static str,
    group: &'static str,
    plural: &'static str,
    namespaced: bool,
    table: bool,
    apply: bool,
) -> BuiltinKind {
    BuiltinKind {
        kind,
        group,
        version: "v1",
        plural,
        namespaced,
        table,
        apply,
    }
}

pub(crate) const BUILTIN_KINDS: [BuiltinKind; 18] = [
    entry("Pod", "", "pods", true, true, true),
    entry("Service", "", "services", true, true, true),
    entry("ConfigMap", "", "configmaps", true, true, true),
    entry("Secret", "", "secrets", true, true, true),
    entry(
        "PersistentVolumeClaim",
        "",
        "persistentvolumeclaims",
        true,
        true,
        true,
    ),
    entry("Namespace", "", "namespaces", false, false, false),
    entry("Node", "", "nodes", false, true, true),
    entry(
        "PersistentVolume",
        "",
        "persistentvolumes",
        false,
        true,
        true,
    ),
    entry("Deployment", "apps", "deployments", true, true, true),
    entry("ReplicaSet", "apps", "replicasets", true, true, false),
    entry("StatefulSet", "apps", "statefulsets", true, true, true),
    entry("DaemonSet", "apps", "daemonsets", true, true, true),
    entry(
        "Ingress",
        "networking.k8s.io",
        "ingresses",
        true,
        true,
        true,
    ),
    entry(
        "EndpointSlice",
        "discovery.k8s.io",
        "endpointslices",
        true,
        false,
        false,
    ),
    entry("Job", "batch", "jobs", true, true, true),
    entry("CronJob", "batch", "cronjobs", true, true, true),
    entry(
        "StorageClass",
        "storage.k8s.io",
        "storageclasses",
        false,
        true,
        true,
    ),
    entry(
        "CustomResourceDefinition",
        "apiextensions.k8s.io",
        "customresourcedefinitions",
        false,
        true,
        false,
    ),
];

pub(crate) fn builtin_kind(kind: &str) -> Option<&'static BuiltinKind> {
    BUILTIN_KINDS.iter().find(|entry| entry.kind == kind)
}

impl BuiltinKind {
    pub(crate) fn api_version(&self) -> String {
        if self.group.is_empty() {
            self.version.to_string()
        } else {
            format!("{}/{}", self.group, self.version)
        }
    }

    pub(crate) fn api_resource(&self) -> ApiResource {
        ApiResource {
            group: self.group.to_string(),
            version: self.version.to_string(),
            api_version: self.api_version(),
            kind: self.kind.to_string(),
            plural: self.plural.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::{NamespaceResourceScope, Resource};
    use std::any::TypeId;

    fn assert_matches<K: Resource>()
    where
        K::Scope: 'static,
    {
        let entry = builtin_kind(K::KIND).unwrap_or_else(|| panic!("{} missing", K::KIND));
        assert_eq!(entry.api_version(), K::API_VERSION, "{}", K::KIND);
        assert_eq!(entry.group, K::GROUP, "{}", K::KIND);
        assert_eq!(entry.plural, K::URL_PATH_SEGMENT, "{}", K::KIND);
        assert_eq!(
            entry.namespaced,
            TypeId::of::<K::Scope>() == TypeId::of::<NamespaceResourceScope>(),
            "{}",
            K::KIND
        );
    }

    #[test]
    fn every_entry_matches_the_kubernetes_api_definition() {
        use k8s_openapi::api::{apps::v1 as apps, batch::v1 as batch, core::v1 as core};
        use k8s_openapi::api::{discovery::v1 as discovery, networking::v1 as networking};
        use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1 as apiextensions;

        assert_matches::<core::Pod>();
        assert_matches::<core::Service>();
        assert_matches::<core::ConfigMap>();
        assert_matches::<core::Secret>();
        assert_matches::<core::PersistentVolumeClaim>();
        assert_matches::<core::Namespace>();
        assert_matches::<core::Node>();
        assert_matches::<core::PersistentVolume>();
        assert_matches::<apps::Deployment>();
        assert_matches::<apps::ReplicaSet>();
        assert_matches::<apps::StatefulSet>();
        assert_matches::<apps::DaemonSet>();
        assert_matches::<networking::Ingress>();
        assert_matches::<discovery::EndpointSlice>();
        assert_matches::<batch::Job>();
        assert_matches::<batch::CronJob>();
        assert_matches::<k8s_openapi::api::storage::v1::StorageClass>();
        assert_matches::<apiextensions::CustomResourceDefinition>();
        // One assertion per entry above; a new entry needs its API definition here.
        assert_eq!(BUILTIN_KINDS.len(), 18);
    }
}
