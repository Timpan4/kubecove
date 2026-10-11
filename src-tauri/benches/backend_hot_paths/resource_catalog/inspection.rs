use super::*;
use kubecove_lib::models::{KubernetesYamlLintResult, KubernetesYamlLintSeverity};

fn parsed_yaml_document(yaml: &str) -> serde_json::Value {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).expect("serialized YAML document");
    serde_json::to_value(value).expect("JSON-shaped Kubernetes document")
}

fn expected_apply_clean_document<T: serde::Serialize>(resource: &T) -> serde_json::Value {
    let mut expected = serde_json::to_value(resource).expect("fixture resource JSON");
    let metadata = expected
        .pointer_mut("/metadata")
        .and_then(serde_json::Value::as_object_mut)
        .expect("fixture metadata");
    for field in [
        "managedFields",
        "uid",
        "resourceVersion",
        "generation",
        "creationTimestamp",
        "selfLink",
    ] {
        assert!(metadata.contains_key(field), "fixture includes {field}");
        metadata.remove(field);
    }
    expected
        .as_object_mut()
        .expect("fixture resource object")
        .remove("status")
        .expect("fixture status");
    expected
}

fn expected_kubectl_document<T: serde::Serialize>(resource: &T) -> serde_json::Value {
    let mut expected = serde_json::to_value(resource).expect("fixture resource JSON");
    let metadata = expected
        .pointer_mut("/metadata")
        .and_then(serde_json::Value::as_object_mut)
        .expect("fixture metadata");
    assert!(metadata.contains_key("managedFields"));
    metadata.remove("managedFields");
    expected
}

fn assert_secret_yaml_redacted(yaml: &str) -> serde_json::Value {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    let document = parsed_yaml_document(yaml);
    let data = document
        .get("data")
        .and_then(serde_json::Value::as_object)
        .expect("Secret data");
    assert_eq!(data.len(), 40);
    let redacted = STANDARD.encode(b"REDACTED");
    for index in 0..40 {
        let key = format!("KEY_{index}");
        assert_eq!(
            data.get(&key).and_then(serde_json::Value::as_str),
            Some(redacted.as_str()),
            "Secret key {key} is redacted"
        );
        let raw = STANDARD.encode(format!("secret-value-and-bytes-{index}"));
        assert!(!yaml.contains(&raw), "Secret key {key} leaked");
    }
    assert_eq!(document["apiVersion"], "v1");
    assert_eq!(document["kind"], "Secret");
    assert_eq!(document["type"], "Opaque");
    assert_eq!(document["metadata"]["name"], "checkout-secret");
    assert_eq!(document["metadata"]["namespace"], "checkout");
    assert_eq!(
        document["metadata"]["annotations"]["kubectl.kubernetes.io/last-applied-configuration"],
        "REDACTED"
    );
    assert!(!yaml.contains("private payload"));
    document
}

fn assert_detail_identity(
    details: &kubecove_lib::models::ResourceDetailsFull,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
    created_at: Option<&str>,
) {
    let summary = &details.summary;
    assert_eq!(
        (
            summary.kind.as_str(),
            summary.cluster.as_str(),
            summary.name.as_str(),
            summary.namespace.as_deref(),
            summary.created_at.as_deref()
        ),
        (kind, "catalog-cluster", name, namespace, created_at)
    );
}

fn assert_metadata<T: serde::Serialize>(actual: &serde_json::Value, resource: &T) {
    assert_eq!(
        actual,
        &serde_json::to_value(resource).expect("fixture metadata")
    );
}

fn assert_kubectl_yaml<T: serde::Serialize>(yaml: &str, resource: &T) {
    assert_eq!(
        parsed_yaml_document(yaml),
        expected_kubectl_document(resource)
    );
}

fn assert_apply_clean_yaml<T: serde::Serialize>(yaml: &str, resource: &T) {
    assert_eq!(
        parsed_yaml_document(yaml),
        expected_apply_clean_document(resource)
    );
}

fn assert_lint_diagnostic(
    result: &KubernetesYamlLintResult,
    severity: KubernetesYamlLintSeverity,
    source: &str,
    field_path: Option<&str>,
    message: &str,
) {
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.severity, severity);
    assert_eq!(diagnostic.source, source);
    assert_eq!(diagnostic.field_path.as_deref(), field_path);
    assert_eq!(diagnostic.message, message);
}

fn assert_schema_diagnostic(
    diagnostic: &kubecove_lib::models::KubernetesYamlLintDiagnostic,
    path: &str,
    message: &str,
) {
    assert_eq!(diagnostic.severity, KubernetesYamlLintSeverity::Error);
    assert_eq!(diagnostic.source, "Kubeconform");
    assert_eq!(diagnostic.field_path.as_deref(), Some(path));
    assert_eq!(diagnostic.message, message);
}

fn assert_schema_note(
    note: &kubecove_lib::models::KubernetesYamlLintStatusNote,
    severity: KubernetesYamlLintSeverity,
    message: &str,
) {
    assert_eq!(note.severity, severity);
    assert_eq!(note.source, "Kubeconform");
    assert_eq!(note.message, message);
}

fn expect_apply_validation_error(
    request: kubecove_lib::models::YamlApplyRequest,
    expected_message: &str,
) {
    let error = editor_apply_validation(request).expect_err("invalid apply request");
    assert_eq!(error.kind, kubecove_lib::models::AppErrorKind::Validation);
    assert_eq!(error.message, expected_message);
}

// R20 plus folded R21: complete documents, normalization, redaction, and detail DTOs.
mod deployment_yaml_document_prepare_40_containers {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn deployment_yaml_document_prepare_40_containers(bencher: codspeed_divan_compat::Bencher) {
        let expected = serialize_typed_document(
            &DOCUMENT_INPUT.deployment,
            YamlViewMode::ApplyClean,
            YamlEncoding::Kyaml,
        )
        .expect("Deployment document");
        assert_apply_clean_yaml(&expected, &DOCUMENT_INPUT.deployment);
        bencher
            .with_inputs(|| DOCUMENT_INPUT.deployment.clone())
            .bench_values(|deployment| {
                black_box(
                    serialize_typed_document(
                        &deployment,
                        YamlViewMode::ApplyClean,
                        YamlEncoding::Kyaml,
                    )
                    .expect("document"),
                )
            });
    }
}

mod secret_yaml_redaction_and_encoding_40_keys {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn secret_yaml_redaction_and_encoding_40_keys(bencher: codspeed_divan_compat::Bencher) {
        let expected = serialize_secret_yaml(
            DOCUMENT_INPUT.secret.clone(),
            YamlViewMode::Kubectl,
            YamlEncoding::Yaml,
        )
        .expect("redacted Secret");
        assert_secret_yaml_redacted(&expected);
        bencher
            .with_inputs(|| DOCUMENT_INPUT.secret.clone())
            .bench_values(|secret| {
                black_box(
                    serialize_secret_yaml(secret, YamlViewMode::Kubectl, YamlEncoding::Yaml)
                        .expect("Secret YAML"),
                )
            });
    }
}

mod dynamic_document_prepare_40_values {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn dynamic_document_prepare_40_values(bencher: codspeed_divan_compat::Bencher) {
        let expected = serialize_dynamic_document(
            &DOCUMENT_INPUT.dynamic_kind,
            &DOCUMENT_INPUT.dynamic_object,
            YamlViewMode::ApplyClean,
            YamlEncoding::Kyaml,
        )
        .expect("dynamic document");
        assert_apply_clean_yaml(&expected, &DOCUMENT_INPUT.dynamic_object);
        bencher
            .with_inputs(|| {
                (
                    DOCUMENT_INPUT.dynamic_kind.clone(),
                    DOCUMENT_INPUT.dynamic_object.clone(),
                )
            })
            .bench_values(|(kind, object)| {
                black_box(
                    serialize_dynamic_document(
                        &kind,
                        &object,
                        YamlViewMode::ApplyClean,
                        YamlEncoding::Kyaml,
                    )
                    .expect("dynamic document"),
                )
            });
    }
}

mod deployment_details_complete_inspection_40_containers {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn deployment_details_complete_inspection_40_containers(
        bencher: codspeed_divan_compat::Bencher,
    ) {
        let expected =
            deployment_details(DOCUMENT_INPUT.deployment.clone()).expect("Deployment details");
        assert_detail_identity(
            &expected,
            "Deployment",
            "checkout-api",
            Some("checkout"),
            Some("2026-09-30T12:00:00+00:00"),
        );
        assert_eq!(expected.summary.ready.as_deref(), Some("4/5"));
        assert_metadata(&expected.metadata, &DOCUMENT_INPUT.deployment.metadata);
        assert_eq!(
            expected.status.as_ref(),
            Some(
                &serde_json::to_value(
                    DOCUMENT_INPUT
                        .deployment
                        .status
                        .as_ref()
                        .expect("Deployment status")
                )
                .expect("Deployment status JSON")
            )
        );
        assert_kubectl_yaml(&expected.yaml, &DOCUMENT_INPUT.deployment);
        assert_eq!(expected.metadata["managedFields"][0]["manager"], "kubectl");
        bencher
            .with_inputs(|| DOCUMENT_INPUT.deployment.clone())
            .bench_values(|deployment| {
                black_box(deployment_details(deployment).expect("Deployment details"))
            });
    }
}

mod configmap_details_complete_inspection_40_keys {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn configmap_details_complete_inspection_40_keys(bencher: codspeed_divan_compat::Bencher) {
        let expected =
            configmap_details(DOCUMENT_INPUT.config_map.clone()).expect("ConfigMap details");
        assert_detail_identity(
            &expected,
            "ConfigMap",
            "checkout-config",
            Some("checkout"),
            Some("2026-09-30T12:00:00+00:00"),
        );
        assert_metadata(&expected.metadata, &DOCUMENT_INPUT.config_map.metadata);
        assert_eq!(expected.status, None);
        assert_kubectl_yaml(&expected.yaml, &DOCUMENT_INPUT.config_map);
        bencher
            .with_inputs(|| DOCUMENT_INPUT.config_map.clone())
            .bench_values(|config_map| {
                black_box(configmap_details(config_map).expect("ConfigMap details"))
            });
    }
}

mod secret_details_discarded_yaml_redaction_and_dto {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn secret_details_discarded_yaml_redaction_and_dto(bencher: codspeed_divan_compat::Bencher) {
        let expected = secret_details(DOCUMENT_INPUT.secret.clone()).expect("Secret details");
        assert_detail_identity(
            &expected,
            "Secret",
            "checkout-secret",
            Some("checkout"),
            Some("2026-09-30T12:00:00+00:00"),
        );
        assert_eq!(expected.status, None);
        let mut expected_metadata =
            serde_json::to_value(&DOCUMENT_INPUT.secret.metadata).expect("Secret metadata");
        expected_metadata["annotations"]["kubectl.kubernetes.io/last-applied-configuration"] =
            serde_json::json!("REDACTED");
        assert_eq!(expected.metadata, expected_metadata);
        assert_secret_yaml_redacted(&expected.yaml);
        bencher
            .with_inputs(|| DOCUMENT_INPUT.secret.clone())
            .bench_values(|secret| black_box(secret_details(secret).expect("Secret details")));
    }
}

mod crd_details_complete_inspection {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn crd_details_complete_inspection(bencher: codspeed_divan_compat::Bencher) {
        let expected = crd_details(DOCUMENT_INPUT.crd.clone()).expect("CRD details");
        assert_detail_identity(
            &expected,
            "CustomResourceDefinition",
            "managedthings0.operator-0.example.com",
            None,
            None,
        );
        let summary = &expected.summary;
        assert_eq!(
            (
                summary.api_version.as_deref(),
                summary.group.as_deref(),
                summary.version.as_deref(),
                summary.plural.as_deref(),
                summary.namespaced
            ),
            (
                Some("apiextensions.k8s.io/v1"),
                Some("apiextensions.k8s.io"),
                Some("v1"),
                Some("customresourcedefinitions"),
                Some(false)
            )
        );
        assert_eq!(expected.status, None);
        assert_metadata(&expected.metadata, &DOCUMENT_INPUT.crd.metadata);
        assert_eq!(
            parsed_yaml_document(&expected.yaml),
            serde_json::to_value(&DOCUMENT_INPUT.crd).expect("CRD document")
        );
        bencher
            .with_inputs(|| DOCUMENT_INPUT.crd.clone())
            .bench_values(|crd| black_box(crd_details(crd).expect("CRD details")));
    }
}

mod dynamic_details_complete_inspection {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn dynamic_details_complete_inspection(bencher: codspeed_divan_compat::Bencher) {
        let expected = dynamic_details(
            DOCUMENT_INPUT.dynamic_kind.clone(),
            DOCUMENT_INPUT.dynamic_object.clone(),
            YamlViewMode::ApplyClean,
            YamlEncoding::Kyaml,
        )
        .expect("dynamic details");
        assert_detail_identity(
            &expected,
            "ManagedThing",
            "checkout-thing",
            Some("checkout"),
            Some("2026-09-30T12:00:00+00:00"),
        );
        assert_eq!(
            (
                expected.summary.api_version.as_deref(),
                expected.summary.group.as_deref(),
                expected.summary.version.as_deref(),
                expected.summary.plural.as_deref(),
                expected.summary.namespaced,
                expected.summary.dynamic
            ),
            (
                Some("example.io/v1"),
                Some("example.io"),
                Some("v1"),
                Some("managedthings"),
                Some(true),
                Some(true)
            )
        );
        assert_eq!(expected.summary.status.as_deref(), Some("Ready: True"));
        assert_metadata(&expected.metadata, &DOCUMENT_INPUT.dynamic_object.metadata);
        assert_eq!(
            expected.status,
            DOCUMENT_INPUT.dynamic_object.data.get("status").cloned()
        );
        assert_apply_clean_yaml(&expected.yaml, &DOCUMENT_INPUT.dynamic_object);
        bencher
            .with_inputs(|| {
                (
                    DOCUMENT_INPUT.dynamic_kind.clone(),
                    DOCUMENT_INPUT.dynamic_object.clone(),
                )
            })
            .bench_values(|(kind, object)| {
                black_box(
                    dynamic_details(kind, object, YamlViewMode::ApplyClean, YamlEncoding::Kyaml)
                        .expect("dynamic details"),
                )
            });
    }
}

// R22: exact owner filtering, complete template serialization, and revision sorting.
mod deployment_revision_history_500_candidates {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn deployment_revision_history_500_candidates(bencher: codspeed_divan_compat::Bencher) {
        let expected = deployment_revisions(REVISION_INPUT.clone());
        assert_eq!(expected.len(), 447);
        assert!(expected.windows(2).all(|rows| {
            rows[0].revision > rows[1].revision
                || (rows[0].revision == rows[1].revision && rows[0].name <= rows[1].name)
        }));
        for index in (0..500).step_by(10) {
            let excluded_name = format!("checkout-{index}-rs");
            assert!(!expected
                .iter()
                .any(|revision| revision.name == excluded_name));
        }
        for excluded_name in ["checkout-1-rs", "checkout-2-rs", "checkout-3-rs"] {
            assert!(!expected
                .iter()
                .any(|revision| revision.name == excluded_name));
        }
        let missing_or_invalid = expected
            .iter()
            .filter(|revision| revision.revision.is_none())
            .map(|revision| revision.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(missing_or_invalid, vec!["checkout-4-rs", "checkout-5-rs"]);
        let top_revision_names = expected
            .iter()
            .filter(|revision| revision.revision == Some(25))
            .map(|revision| revision.name.as_str())
            .collect::<Vec<_>>();
        // Every 25th candidate has revision 25; none has a rejected owner.
        assert_eq!(top_revision_names.len(), 20);
        assert!(top_revision_names
            .windows(2)
            .all(|names| names[0] < names[1]));
        let latest = expected.first().expect("highest revision");
        assert_eq!(latest.name, "checkout-124-rs");
        let source = REVISION_INPUT
            .replica_sets
            .iter()
            .find(|replica_set| replica_set.metadata.name.as_deref() == Some(latest.name.as_str()))
            .expect("latest ReplicaSet fixture");
        let parsed_template: serde_yaml::Value =
            serde_yaml::from_str(&latest.pod_template_yaml).expect("serialized Pod template");
        assert_eq!(
            serde_json::to_value(parsed_template).expect("Pod template JSON"),
            serde_json::to_value(&source.spec.as_ref().expect("ReplicaSet spec").template)
                .expect("fixture Pod template JSON")
        );
        bencher
            .with_inputs(|| REVISION_INPUT.clone())
            .bench_values(|fixture| black_box(deployment_revisions(fixture)));
    }
}

// R23: local lint, apply validation outcomes, and captured schema output projection.
mod editor_local_lint_multiple_documents {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn editor_local_lint_multiple_documents(bencher: codspeed_divan_compat::Bencher) {
        let expected = editor_local_lint(EDITOR_INPUT.multiple_documents.clone());
        assert_eq!(expected.notes.len(), 0);
        assert_lint_diagnostic(
            &expected,
            KubernetesYamlLintSeverity::Error,
            "YAML",
            None,
            "YAML apply accepts exactly one document",
        );

        for (request, path, message) in [
            (
                EDITOR_INPUT.api_version_mismatch.clone(),
                "apiVersion",
                "apiVersion must match selected resource: expected v1, got apps/v1.",
            ),
            (
                EDITOR_INPUT.kind_mismatch.clone(),
                "kind",
                "kind must match selected resource: expected ConfigMap, got Service.",
            ),
            (
                EDITOR_INPUT.mismatch.clone(),
                "metadata.name",
                "metadata.name must match selected resource: expected checkout-config, got another-config.",
            ),
            (
                EDITOR_INPUT.namespace_mismatch.clone(),
                "metadata.namespace",
                "metadata.namespace must match selected resource: expected checkout, got another-namespace.",
            ),
        ] {
            let lint = editor_local_lint(request);
            assert_lint_diagnostic(
                &lint,
                KubernetesYamlLintSeverity::Error,
                "Kubernetes",
                Some(path),
                message,
            );
        }
        let secret_lint = editor_local_lint(EDITOR_INPUT.secret.clone());
        assert_lint_diagnostic(
            &secret_lint,
            KubernetesYamlLintSeverity::Warning,
            "Kubernetes",
            Some("data"),
            "v1 Secret apply is blocked because redacted values can corrupt live data.",
        );
        bencher
            .with_inputs(|| EDITOR_INPUT.multiple_documents.clone())
            .bench_values(|request| black_box(editor_local_lint(request)));
    }
}

mod editor_apply_validation_valid {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn editor_apply_validation_valid(bencher: codspeed_divan_compat::Bencher) {
        let (target, manifest) =
            editor_apply_validation(EDITOR_INPUT.valid.clone()).expect("valid apply request");
        assert_eq!(target.cluster_context, "catalog-cluster");
        assert_eq!(target.kind, "ConfigMap");
        assert_eq!(target.api_version.as_deref(), Some("v1"));
        assert_eq!(target.name, "checkout-config");
        assert_eq!(target.namespace.as_deref(), Some("checkout"));
        assert_eq!(
            manifest,
            serde_json::json!({
                "apiVersion":"v1",
                "kind":"ConfigMap",
                "metadata":{"name":"checkout-config","namespace":"checkout"},
                "data":{"LOG_LEVEL":"info"}
            })
        );
        bencher
            .with_inputs(|| EDITOR_INPUT.valid.clone())
            .bench_values(|request| black_box(editor_apply_validation(request)));
    }
}

mod editor_apply_validation_target_mismatch {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn editor_apply_validation_target_mismatch(bencher: codspeed_divan_compat::Bencher) {
        expect_apply_validation_error(
            EDITOR_INPUT.api_version_mismatch.clone(),
            "apiVersion must match selected resource: expected v1, got apps/v1",
        );
        expect_apply_validation_error(
            EDITOR_INPUT.kind_mismatch.clone(),
            "kind must match selected resource: expected ConfigMap, got Service",
        );
        expect_apply_validation_error(
            EDITOR_INPUT.mismatch.clone(),
            "metadata.name must match selected resource: expected checkout-config, got another-config",
        );
        expect_apply_validation_error(
            EDITOR_INPUT.namespace_mismatch.clone(),
            "metadata.namespace must match selected resource: expected checkout, got another-namespace",
        );
        bencher
            .with_inputs(|| EDITOR_INPUT.mismatch.clone())
            .bench_values(|request| black_box(editor_apply_validation(request)));
    }
}

mod editor_apply_validation_secret_rejection {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn editor_apply_validation_secret_rejection(bencher: codspeed_divan_compat::Bencher) {
        expect_apply_validation_error(
            EDITOR_INPUT.secret.clone(),
            "YAML apply is disabled for v1 Secrets because redacted values can corrupt data",
        );
        bencher
            .with_inputs(|| EDITOR_INPUT.secret.clone())
            .bench_values(|request| black_box(editor_apply_validation(request)));
    }
}

mod editor_apply_validation_empty_comment_null {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn editor_apply_validation_empty_comment_null(bencher: codspeed_divan_compat::Bencher) {
        for request in &EDITOR_INPUT.empty_comment_null {
            expect_apply_validation_error(request.clone(), "YAML document is empty");
        }
        bencher
            .with_inputs(|| EDITOR_INPUT.empty_comment_null.clone())
            .bench_values(|requests| {
                black_box(
                    requests
                        .into_iter()
                        .map(editor_apply_validation)
                        .collect::<Vec<_>>(),
                )
            });
    }
}

mod editor_apply_validation_malformed_yaml {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn editor_apply_validation_malformed_yaml(bencher: codspeed_divan_compat::Bencher) {
        let error = editor_apply_validation(EDITOR_INPUT.malformed.clone())
            .expect_err("malformed YAML apply request");
        assert_eq!(error.kind, kubecove_lib::models::AppErrorKind::Validation);
        bencher
            .with_inputs(|| EDITOR_INPUT.malformed.clone())
            .bench_values(|request| black_box(editor_apply_validation(request)));
    }
}

mod editor_schema_output_projection_valid_invalid_skipped_error {
    use super::*;
    #[codspeed_divan_compat::bench]
    fn editor_schema_output_projection_valid_invalid_skipped_error(
        bencher: codspeed_divan_compat::Bencher,
    ) {
        let expected = schema_output_projection(&SCHEMA_INPUT);
        assert_eq!(expected.diagnostics.len(), 3);
        assert_schema_diagnostic(
            &expected.diagnostics[0],
            "spec.replicas",
            "replicas: expected integer",
        );
        assert_schema_diagnostic(
            &expected.diagnostics[1],
            "spec.template~/config",
            "template~/config: must be an object",
        );
        assert_schema_diagnostic(
            &expected.diagnostics[2],
            "apiVersion",
            "schema check failed",
        );
        assert_eq!(expected.notes.len(), 3);
        assert_schema_note(
            &expected.notes[0],
            KubernetesYamlLintSeverity::Info,
            "Schema validation incomplete for ManagedThing: no schema found",
        );
        assert_schema_note(
            &expected.notes[1],
            KubernetesYamlLintSeverity::Warning,
            "Schema validation incomplete for Service: schema registry unavailable",
        );
        assert_schema_note(
            &expected.notes[2],
            KubernetesYamlLintSeverity::Warning,
            "Schema validation incomplete for FutureKind: unsupported future status",
        );

        let unreadable = SchemaOutputFixture {
            stdout: b"not JSON".to_vec(),
            stderr: Vec::new(),
        };
        let unreadable_result = schema_output_projection(&unreadable);
        assert!(unreadable_result.diagnostics.is_empty());
        assert_eq!(unreadable_result.notes.len(), 1);
        assert_schema_note(
            &unreadable_result.notes[0],
            KubernetesYamlLintSeverity::Warning,
            "Kubeconform returned unreadable JSON output.",
        );
        bencher
            .with_inputs(|| SCHEMA_INPUT.clone())
            .bench_values(|fixture| black_box(schema_output_projection(&fixture)));
    }
}
