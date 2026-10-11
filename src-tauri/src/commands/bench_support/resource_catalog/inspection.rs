use super::summaries;
use crate::{
    commands::{helpers, kubeconfig, resources},
    models::{
        AppError, ClusterContext, DiscoveredResourceKind, KubernetesYamlLintResult,
        ResourceDetailsFull, YamlApplyRequest, YamlApplyTarget, YamlEncoding, YamlViewMode,
    },
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, ReplicaSet},
        core::v1::{ConfigMap, Secret},
    },
    apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
};
use kube::api::DynamicObject;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct YamlDocumentFixture {
    pub deployment: Deployment,
    pub config_map: ConfigMap,
    pub secret: Secret,
    pub crd: CustomResourceDefinition,
    pub dynamic_kind: DiscoveredResourceKind,
    pub dynamic_object: DynamicObject,
}

#[derive(Clone)]
pub struct KubeconfigFixture {
    pub sources: Vec<String>,
}

#[derive(Clone)]
pub struct RevisionFixture {
    pub replica_sets: Vec<ReplicaSet>,
    pub deployment_uid: String,
}

#[derive(Clone)]
pub struct EditorFixture {
    pub valid: YamlApplyRequest,
    pub mismatch: YamlApplyRequest,
    pub api_version_mismatch: YamlApplyRequest,
    pub kind_mismatch: YamlApplyRequest,
    pub namespace_mismatch: YamlApplyRequest,
    pub secret: YamlApplyRequest,
    pub multiple_documents: YamlApplyRequest,
    pub empty_comment_null: Vec<YamlApplyRequest>,
    pub malformed: YamlApplyRequest,
}

#[derive(Clone)]
pub struct SchemaOutputFixture {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("valid inspection benchmark object")
}

pub fn yaml_document_fixture() -> YamlDocumentFixture {
    let containers = (0..40)
        .map(|index| {
            json!({
                "name":format!("worker-{index}"),
                "image":format!("ghcr.io/kubecove/worker-{index}:1.0.0"),
                "env":[{"name":"WORKER_INDEX","value":index.to_string()},{"name":"RUST_LOG","value":"info"}],
                "resources":{"requests":{"cpu":format!("{}m",100 + index),"memory":"128Mi"},"limits":{"memory":"256Mi"}}
            })
        })
        .collect::<Vec<_>>();
    let deployment = parse(json!({
        "apiVersion":"apps/v1","kind":"Deployment",
        "metadata":{"name":"checkout-api","namespace":"checkout","uid":"deployment-uid","resourceVersion":"17","generation":3,"creationTimestamp":"2026-09-30T12:00:00Z","selfLink":"/apis/apps/v1/namespaces/checkout/deployments/checkout-api","labels":{"app.kubernetes.io/name":"checkout","app.kubernetes.io/part-of":"payments"},"managedFields":[{"manager":"kubectl","operation":"Apply","apiVersion":"apps/v1","time":"2026-09-30T12:00:00Z","fieldsType":"FieldsV1","fieldsV1":{"f:spec":{}}}]},
        "spec":{"replicas":5,"selector":{"matchLabels":{"app.kubernetes.io/name":"checkout"}},"template":{"metadata":{"labels":{"app.kubernetes.io/name":"checkout"}},"spec":{"containers":containers}}},
        "status":{"replicas":5,"readyReplicas":4,"availableReplicas":4}
    }));
    let config_data = (0..40)
        .map(|index| {
            (
                format!("KEY_{index}"),
                Value::String(format!("value-{index}")),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let config_map: ConfigMap = parse(json!({
        "apiVersion":"v1","kind":"ConfigMap","metadata":{"name":"checkout-config","namespace":"checkout","creationTimestamp":"2026-09-30T12:00:00Z","managedFields":[{"manager":"kubectl","operation":"Apply"}]},"data":config_data
    }));
    let secret_data = (0..40)
        .map(|index| {
            let raw_value = format!("secret-value-and-bytes-{index}");
            (
                format!("KEY_{index}"),
                Value::String(STANDARD.encode(raw_value.as_bytes())),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let secret: Secret = parse(json!({
        "apiVersion":"v1","kind":"Secret","metadata":{"name":"checkout-secret","namespace":"checkout","creationTimestamp":"2026-09-30T12:00:00Z","annotations":{"kubectl.kubernetes.io/last-applied-configuration":"private payload"},"managedFields":[{"manager":"kubectl","operation":"Apply"}]},"type":"Opaque","data":secret_data
    }));
    let crd = summaries::crd_definition_fixture(1)
        .pop()
        .expect("CRD detail fixture");
    let dynamic_kind = DiscoveredResourceKind {
        group: "example.io".to_string(),
        version: "v1".to_string(),
        api_version: "example.io/v1".to_string(),
        kind: "ManagedThing".to_string(),
        plural: "managedthings".to_string(),
        short_names: vec!["mt".to_string()],
        namespaced: true,
    };
    let dynamic_object = parse(json!({
        "apiVersion":"example.io/v1","kind":"ManagedThing",
        "metadata":{"name":"checkout-thing","namespace":"checkout","uid":"thing-uid","resourceVersion":"23","generation":4,"creationTimestamp":"2026-09-30T12:00:00Z","selfLink":"/apis/example.io/v1/namespaces/checkout/managedthings/checkout-thing","labels":{"app":"checkout"},"managedFields":[{"manager":"controller","operation":"Apply"}]},
        "spec":{"replicas":5,"values":(0..40).map(|index| json!({"key":format!("key-{index}"),"value":format!("value-{index}")})).collect::<Vec<_>>()},
        "status":{"conditions":[{"type":"Ready","status":"True","reason":"Ready","message":"resource is ready"}]}
    }));
    YamlDocumentFixture {
        deployment,
        config_map,
        secret,
        crd,
        dynamic_kind,
        dynamic_object,
    }
}

pub fn serialize_secret_yaml(
    secret: Secret,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    resources::serialize_secret_resource_document(secret, mode, encoding)
}

pub fn serialize_typed_document<T: serde::Serialize>(
    value: &T,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    helpers::serialize_resource_document(value, mode, encoding)
}

pub fn serialize_dynamic_document(
    resource_kind: &DiscoveredResourceKind,
    object: &DynamicObject,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    resources::serialize_dynamic_resource_document(resource_kind, object, mode, encoding)
}

pub fn deployment_details(deployment: Deployment) -> Result<ResourceDetailsFull, AppError> {
    let yaml = serialize_typed_document(
        &deployment,
        YamlViewMode::default(),
        YamlEncoding::default(),
    )?;
    let name = deployment.metadata.name.clone().unwrap_or_default();
    let namespace = deployment.metadata.namespace.clone();
    resources::deployment_details_from_fetched(
        "catalog-cluster".to_string(),
        name,
        namespace,
        deployment,
        yaml,
    )
}

pub fn configmap_details(config_map: ConfigMap) -> Result<ResourceDetailsFull, AppError> {
    let yaml = serialize_typed_document(
        &config_map,
        YamlViewMode::default(),
        YamlEncoding::default(),
    )?;
    let name = config_map.metadata.name.clone().unwrap_or_default();
    let namespace = config_map.metadata.namespace.clone();
    resources::configmap_details_from_fetched(
        "catalog-cluster".to_string(),
        name,
        namespace,
        config_map,
        yaml,
    )
}

pub fn secret_details(secret: Secret) -> Result<ResourceDetailsFull, AppError> {
    // Match get_resource_details: fetch-and-serialize first, then the Secret detail path discards it.
    let discarded =
        serialize_typed_document(&secret, YamlViewMode::default(), YamlEncoding::default())?;
    let name = secret.metadata.name.clone().unwrap_or_default();
    let namespace = secret.metadata.namespace.clone();
    resources::secret_details_from_fetched(
        "catalog-cluster".to_string(),
        name,
        namespace,
        secret,
        discarded,
    )
}

pub fn crd_details(crd: CustomResourceDefinition) -> Result<ResourceDetailsFull, AppError> {
    let yaml = serde_yaml::to_string(&crd).map_err(|error| {
        AppError::new(
            error.to_string(),
            crate::models::AppErrorKind::Serialization,
        )
    })?;
    resources::crd_details_from_fetched("catalog-cluster".to_string(), crd, yaml)
}

pub fn dynamic_details(
    resource_kind: DiscoveredResourceKind,
    object: DynamicObject,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<ResourceDetailsFull, AppError> {
    resources::dynamic_resource_details_from_object(
        "catalog-cluster".to_string(),
        resource_kind,
        object,
        mode,
        encoding,
    )
}

pub fn kubeconfig_source_fixture() -> KubeconfigFixture {
    let sources = (0..3)
        .map(|source| {
            let context = if source < 2 { "shared-context" } else { "context-2" };
            let cluster = format!("cluster-{source}");
            let user = format!("user-{source}");
            format!(
                "apiVersion: v1\nkind: Config\ncurrent-context: {context}\nclusters:\n- name: {cluster}\n  cluster:\n    server: https://{cluster}.example.invalid\nusers:\n- name: {user}\n  user: {{}}\ncontexts:\n- name: {context}\n  context:\n    cluster: {cluster}\n    user: {user}\n    namespace: tenant-{source}\n"
            )
        })
        .collect();
    KubeconfigFixture { sources }
}

pub fn kubeconfig_context_projection(
    fixture: &KubeconfigFixture,
) -> Result<Vec<ClusterContext>, AppError> {
    let sources = fixture
        .sources
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let config = kubeconfig::kubeconfig_from_source_texts(&sources)?;
    Ok(kubeconfig::cluster_contexts_from_kubeconfig(&config))
}

pub fn revision_fixture() -> RevisionFixture {
    let mut replica_sets = Vec::with_capacity(500);
    for index in 0_usize..500 {
        let wrong_owner = index.is_multiple_of(10);
        let mut annotations = json!({
            "deployment.kubernetes.io/revision":format!("{}",index % 25 + 1),
            "kubernetes.io/change-cause":format!("release {index}")
        });
        if index == 4 {
            annotations
                .as_object_mut()
                .expect("revision annotations object")
                .remove("deployment.kubernetes.io/revision");
        } else if index == 5 {
            annotations["deployment.kubernetes.io/revision"] = json!("invalid");
        }
        let mut owner = json!({
            "apiVersion":"apps/v1",
            "kind":"Deployment",
            "name":"checkout-api",
            "uid":if wrong_owner {"other-deployment-uid"} else {"deployment-uid"},
            "controller":true
        });
        match index {
            1 => owner["controller"] = json!(false),
            2 => owner["kind"] = json!("StatefulSet"),
            3 => owner["apiVersion"] = json!("apps/v1beta1"),
            _ => {}
        }
        let value = json!({
            "apiVersion":"apps/v1","kind":"ReplicaSet",
            "metadata":{"name":format!("checkout-{index}-rs"),"namespace":"checkout","uid":format!("rs-{index}"),"creationTimestamp":"2026-09-30T12:00:00Z",
                "annotations":annotations,
                "ownerReferences":[owner]},
            "spec":{"replicas":1,"selector":{"matchLabels":{"app":"checkout"}},"template":{"metadata":{"labels":{"app":"checkout","revision":format!("{}",index % 25 + 1)}},"spec":{"containers":[{"name":"checkout","image":format!("ghcr.io/example/checkout:{index}"),"env":[{"name":"REVISION","value":index.to_string()}]}]}}}
        });
        replica_sets.push(parse(value));
    }
    RevisionFixture {
        replica_sets,
        deployment_uid: "deployment-uid".to_string(),
    }
}

pub fn deployment_revisions(fixture: RevisionFixture) -> Vec<crate::models::DeploymentRevision> {
    resources::deployment_revisions_from_replica_sets(fixture.replica_sets, &fixture.deployment_uid)
}

fn apply_request(
    kind: &str,
    name: &str,
    namespace: Option<&str>,
    yaml: String,
) -> YamlApplyRequest {
    YamlApplyRequest {
        cluster_context: "catalog-cluster".to_string(),
        kubeconfig_env_var: None,
        kind: kind.to_string(),
        api_version: Some("v1".to_string()),
        group: None,
        version: None,
        plural: None,
        namespaced: Some(namespace.is_some()),
        name: name.to_string(),
        namespace: namespace.map(str::to_string),
        yaml,
        yaml_encoding: YamlEncoding::Yaml,
        force_conflicts: false,
    }
}

pub fn editor_fixture() -> EditorFixture {
    let valid = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: checkout-config\n  namespace: checkout\ndata:\n  LOG_LEVEL: info\n".to_string();
    let mismatch = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: another-config\n  namespace: checkout\ndata:\n  LOG_LEVEL: info\n".to_string();
    let api_version_mismatch = valid.replace("apiVersion: v1", "apiVersion: apps/v1");
    let kind_mismatch = valid.replace("kind: ConfigMap", "kind: Service");
    let namespace_mismatch = valid.replace("namespace: checkout", "namespace: another-namespace");
    let secret = "apiVersion: v1\nkind: Secret\nmetadata:\n  name: checkout-secret\n  namespace: checkout\ndata:\n  password: REDACTED\n".to_string();
    let multiple_documents = format!("{valid}---\n{mismatch}");
    let empty_comment_null = [
        String::new(),
        "# saved draft\n".to_string(),
        "null\n".to_string(),
    ]
    .into_iter()
    .map(|yaml| apply_request("ConfigMap", "checkout-config", Some("checkout"), yaml))
    .collect();
    EditorFixture {
        valid: apply_request("ConfigMap", "checkout-config", Some("checkout"), valid),
        mismatch: apply_request("ConfigMap", "checkout-config", Some("checkout"), mismatch),
        api_version_mismatch: apply_request(
            "ConfigMap",
            "checkout-config",
            Some("checkout"),
            api_version_mismatch,
        ),
        kind_mismatch: apply_request(
            "ConfigMap",
            "checkout-config",
            Some("checkout"),
            kind_mismatch,
        ),
        namespace_mismatch: apply_request(
            "ConfigMap",
            "checkout-config",
            Some("checkout"),
            namespace_mismatch,
        ),
        secret: apply_request("Secret", "checkout-secret", Some("checkout"), secret),
        multiple_documents: apply_request(
            "ConfigMap",
            "checkout-config",
            Some("checkout"),
            multiple_documents,
        ),
        empty_comment_null,
        malformed: apply_request(
            "ConfigMap",
            "checkout-config",
            Some("checkout"),
            "apiVersion: v1\nkind: ConfigMap\nmetadata: [\n".to_string(),
        ),
    }
}

pub fn editor_local_lint(request: YamlApplyRequest) -> KubernetesYamlLintResult {
    resources::lint_kubernetes_yaml_request(request)
}

pub fn editor_apply_validation(
    request: YamlApplyRequest,
) -> Result<(YamlApplyTarget, Value), AppError> {
    let validated = resources::validate_yaml_apply(request)?;
    Ok((validated.target, validated.manifest))
}

pub fn schema_output_fixture() -> SchemaOutputFixture {
    let stdout = json!({"resources":[
        {"kind":"ConfigMap","status":"statusValid"},
        {"kind":"Deployment","status":"statusInvalid","validationErrors":[{"path":"/spec/replicas","msg":"expected integer"},{"path":"/spec/ignored","msg":"ignored later validation error"}]},
        {"kind":"ManagedThing","status":"statusInvalid","validationErrors":[{"path":"/spec/template~0~1config","msg":"must be an object"},{"path":"/spec/ignored","msg":"ignored later validation error"}]},
        {"kind":"MissingErrors","status":"statusInvalid","msg":"schema check failed"},
        {"kind":"ManagedThing","status":"statusSkipped","msg":"no schema found"},
        {"kind":"Service","status":"statusError","msg":"schema registry unavailable"},
        {"kind":"FutureKind","status":"statusFuture","msg":"unsupported future status"}
    ]})
    .to_string()
    .into_bytes();
    SchemaOutputFixture {
        stdout,
        stderr: b"captured schema diagnostics".to_vec(),
    }
}

pub fn schema_output_projection(fixture: &SchemaOutputFixture) -> KubernetesYamlLintResult {
    let mut result = KubernetesYamlLintResult {
        diagnostics: Vec::new(),
        notes: Vec::new(),
    };
    resources::collect_kubeconform_bytes(&fixture.stdout, true, &fixture.stderr, &mut result);
    result
}
