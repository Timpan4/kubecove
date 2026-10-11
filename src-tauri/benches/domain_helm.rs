use codspeed_divan_compat::{black_box, Bencher};
use kubecove_lib::commands::bench_support::domain_catalog::domain_helm;
use kubecove_lib::models::AppErrorKind;

mod helm_storage_records_to_latest_releases {
    use super::*;

    // R25 and folded R26: decode every Secret/ConfigMap record, construct its
    // full summaries, select latest revisions, and drop discarded payloads.
    #[codspeed_divan_compat::bench]
    fn helm_storage_records_to_latest_releases(bencher: Bencher) {
        let fixture = domain_helm::storage_records();
        assert_eq!(fixture.0.len() + fixture.1.len(), 250);
        let expected = domain_helm::benchmark_storage_records(fixture.0.clone(), fixture.1.clone())
            .expect("storage fixtures decode into records");
        assert_eq!(expected.len(), 51);
        let malformed = fixture
            .0
            .iter()
            .find(|secret| {
                secret.metadata.name.as_deref() == Some("sh.helm.release.v1.release-48.v5")
            })
            .expect("malformed latest Secret fixture");
        assert!(domain_helm::invalid_storage_error_is_retained(
            malformed.clone()
        ));
        assert_eq!(
            domain_helm::storage_record_error(malformed.clone()),
            Some((
                AppErrorKind::Serialization,
                "Helm release encoding is not base64".to_string()
            ))
        );
        let latest = expected
            .iter()
            .find(|summary| summary.name == "release-48")
            .expect("latest malformed record summary is retained");
        assert_eq!(latest.revision, Some(5));
        assert_eq!(latest.storage_kind, "Secret");
        let namespace_collision = expected
            .iter()
            .find(|summary| summary.name == "release-00" && summary.namespace == "other")
            .expect("same release name in another namespace remains distinct");
        assert_eq!(namespace_collision.revision, Some(1));
        let tied = expected
            .iter()
            .find(|summary| summary.name == "release-00" && summary.namespace == "payments")
            .expect("release with equal latest revisions remains present");
        assert_eq!(tied.revision, Some(4));
        assert_eq!(tied.storage_kind, "Secret");
        let missing_revision = expected
            .iter()
            .find(|summary| summary.name == "release-49" && summary.namespace == "payments")
            .expect("revisionless latest record does not replace a numbered revision");
        assert_eq!(missing_revision.revision, Some(4));
        assert_eq!(missing_revision.storage_kind, "ConfigMap");
        assert!(expected.windows(2).all(|pair| {
            (&pair[0].namespace, &pair[0].name) <= (&pair[1].namespace, &pair[1].name)
        }));
        let normal_secret = fixture
            .0
            .iter()
            .find(|secret| {
                secret.metadata.name.as_deref() == Some("sh.helm.release.v1.release-00.v1")
            })
            .expect("normal Secret storage fixture");
        let (summary, values, manifest, release, decode_error) =
            domain_helm::storage_record_projection(normal_secret.clone())
                .expect("full release record projects");
        assert_eq!(summary.chart.as_deref(), Some("checkout-1.0.0"));
        assert!(values.has_values);
        assert_eq!(values.value_count, 3);
        assert_eq!(values.top_level_keys, ["image", "password", "replicaCount"]);
        assert_eq!(manifest.resource_count, 2);
        assert_eq!(manifest.resources.len(), 2);
        assert!(!manifest.truncated);
        assert!(release.is_some());
        assert!(!decode_error);
        let safe_output = serde_json::to_string(&(&summary, &values, &manifest))
            .expect("safe record DTOs serialize");
        assert!(!safe_output.contains("must-not-be-in-safe-release-metadata"));
        let identity = domain_helm::storage_record_projection(
            domain_helm::storage_identity_precedence_record(),
        )
        .expect("identity precedence fixture decodes");
        assert_eq!(identity.0.name, "decoded-name");
        assert_eq!(
            identity.0.storage_name,
            "sh.helm.release.v1.storage-name.v1"
        );

        bencher
            .with_inputs(|| (fixture.0.clone(), fixture.1.clone()))
            .bench_values(|(secrets, configmaps)| {
                black_box(
                    domain_helm::benchmark_storage_records(secrets, configmaps)
                        .expect("fixture records remain valid inputs"),
                )
            });
    }
}

mod helm_storage_record_decode_bounds {
    use super::*;

    // R25: exercise the actual stored-size and expanded-size rejection paths.
    #[codspeed_divan_compat::bench]
    fn helm_storage_record_decode_bounds(bencher: Bencher) {
        let records = domain_helm::storage_bound_records();
        assert_eq!(
            records[0].data.as_ref().unwrap()["release"].0.len(),
            domain_helm::STORAGE_RECORD_MAX_BYTES + 1
        );
        assert!(
            records[1].data.as_ref().unwrap()["release"].0.len()
                < domain_helm::STORAGE_RECORD_MAX_BYTES
        );
        assert!(records
            .iter()
            .cloned()
            .all(domain_helm::invalid_storage_error_is_retained));
        assert_eq!(
            records
                .iter()
                .cloned()
                .map(domain_helm::storage_record_error)
                .collect::<Vec<_>>(),
            vec![
                Some((
                    AppErrorKind::Validation,
                    "Helm release storage exceeds the Kubernetes 1 MiB limit".to_string()
                )),
                Some((
                    AppErrorKind::Validation,
                    "Decoded Helm release exceeds the 32 MiB limit".to_string()
                ))
            ]
        );
        let summaries = domain_helm::benchmark_storage_records(records.clone(), Vec::new())
            .expect("decode failures stay attached to records");
        assert_eq!(summaries.len(), 2);
        assert!(summaries
            .iter()
            .any(|summary| summary.name == "oversized-stored"));
        assert!(summaries
            .iter()
            .any(|summary| summary.name == "oversized-decoded"));

        bencher
            .with_inputs(|| records.clone())
            .bench_values(|records| {
                black_box(
                    domain_helm::benchmark_storage_records(records, Vec::new())
                        .expect("decode failures remain per-record"),
                )
            });
    }
}

mod helm_manifest_parse_mixed_250_documents {
    use super::*;

    // R27: retain the caller's whole-document YAML parse over mixed resource
    // kinds, embedded config, empty documents, and malformed/non-resource docs.
    #[codspeed_divan_compat::bench]
    fn helm_manifest_parse_mixed_250_documents(bencher: Bencher) {
        let manifest = domain_helm::manifest_fixture();
        let parsed = domain_helm::parse_manifest(&manifest);
        assert_eq!(parsed.len(), 250);
        assert_eq!(
            parsed.first().and_then(|row| row.name.as_deref()),
            Some("resource-000")
        );
        assert_eq!(
            parsed.last().and_then(|row| row.name.as_deref()),
            Some("resource-249")
        );
        let empty_documents = domain_helm::parse_manifest("---\n---");
        let malformed_document = domain_helm::parse_manifest("apiVersion: apps/v1\nkind: [");
        assert!(empty_documents.is_empty());
        assert!(malformed_document.is_empty());
        let preview = domain_helm::manifest_summary(&parsed);
        assert_eq!(preview.resource_count, 250);
        assert_eq!(preview.resources.len(), 50);
        assert!(preview.truncated);

        bencher
            .with_inputs(|| manifest.clone())
            .bench_values(|manifest| black_box(domain_helm::parse_manifest(black_box(&manifest))));
    }
}

mod helm_reconciliation_prepare {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn helm_reconciliation_prepare(bencher: Bencher) {
        let (refs, discovered) = domain_helm::reconciliation_inputs();
        let expected = domain_helm::reconciliation_prepare(&refs, &discovered);
        assert_eq!(expected.discovered_index_size, 1_000);
        assert_eq!(expected.valid_identity_count, 250);
        let namespace_fallback_ref = refs
            .iter()
            .find(|manifest_ref| {
                manifest_ref.namespace.is_none()
                    && discovered.iter().any(|kind| {
                        kind.namespaced
                            && manifest_ref.api_version.as_deref()
                                == Some(kind.api_version.as_str())
                            && manifest_ref.kind.as_deref() == Some(kind.kind.as_str())
                    })
            })
            .expect("fixture includes namespaced ref without explicit namespace");
        assert_eq!(
            domain_helm::reconciliation_identity_namespace(
                namespace_fallback_ref,
                "payments",
                &discovered,
            )
            .as_deref(),
            Some("payments")
        );
        assert_eq!(expected.unavailable_rows.len(), 2);
        assert!(expected.unavailable_rows.iter().all(|row| {
            row.status == kubecove_lib::models::HelmReconciliationStatus::Unavailable
                && row.in_manifest
                && !row.explicit_helm_label
                && row.live_resource.is_none()
        }));
        assert_eq!(
            expected.unavailable_rows[0].status_message,
            "operator.example.io/v1 MissingDiscoveryKind is not available through discovery"
        );
        assert_eq!(
            expected.unavailable_rows[1].status_message,
            "manifest resource is missing name"
        );
        assert!(!expected.scan_kinds.is_empty());
        assert!(expected
            .scan_kinds
            .iter()
            .all(|scan_kind| discovered.iter().any(|kind| {
                kind.api_version == scan_kind.api_version && kind.kind == scan_kind.kind
            })));

        bencher
            .with_inputs(|| (refs.clone(), discovered.clone()))
            .bench_values(|(refs, discovered)| {
                black_box(domain_helm::reconciliation_prepare(&refs, &discovered))
            });
    }
}

mod helm_reconciliation_assemble {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn helm_reconciliation_assemble(bencher: Bencher) {
        let fixture = domain_helm::reconciliation_assembly_inputs();
        let expected = domain_helm::reconciliation_assemble(
            fixture.0.clone(),
            fixture.1.clone(),
            fixture.2.clone(),
            fixture.3.clone(),
        );
        assert_eq!(expected.resources.len(), 253);
        assert_eq!(expected.warnings, vec!["scan incomplete"]);
        assert_eq!(expected.totals.tracked, 50);
        assert_eq!(expected.totals.unavailable, 53);
        assert_eq!(expected.totals.unlabeled_live, 50);
        assert_eq!(expected.totals.missing, 50);
        assert_eq!(expected.totals.label_only, 50);
        assert!(expected.resources.windows(2).all(|pair| {
            (
                pair[0].namespace.as_deref().unwrap_or_default(),
                pair[0].kind.as_deref().unwrap_or_default(),
                pair[0].name.as_deref().unwrap_or_default(),
                pair[0].api_version.as_deref().unwrap_or_default(),
            ) <= (
                pair[1].namespace.as_deref().unwrap_or_default(),
                pair[1].kind.as_deref().unwrap_or_default(),
                pair[1].name.as_deref().unwrap_or_default(),
                pair[1].api_version.as_deref().unwrap_or_default(),
            )
        }));

        bencher
            .with_inputs(|| {
                (
                    fixture.0.clone(),
                    fixture.1.clone(),
                    fixture.2.clone(),
                    fixture.3.clone(),
                )
            })
            .bench_values(|(summary, rows, unavailable, warnings)| {
                black_box(domain_helm::reconciliation_assemble(
                    summary,
                    rows,
                    unavailable,
                    warnings,
                ))
            });
    }
}

mod helm_release_list_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn helm_release_list_json_encoding(bencher: Bencher) {
        let fixture = domain_helm::storage_records();
        let releases = domain_helm::benchmark_storage_records(fixture.0, fixture.1)
            .expect("storage fixtures decode");
        let json = domain_helm::encode_release_list(releases.clone());
        assert!(json.contains("release-00"));
        assert!(!json.contains("must-not-be-in-safe-release-metadata"));
        bencher
            .with_inputs(|| releases.clone())
            .bench_values(|releases| black_box(domain_helm::encode_release_list(releases)));
    }
}

mod helm_manifest_summary_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn helm_manifest_summary_json_encoding(bencher: Bencher) {
        let manifest = domain_helm::manifest_fixture();
        let resources = domain_helm::parse_manifest(&manifest);
        let summary = domain_helm::manifest_summary(&resources);
        let json = domain_helm::encode(&summary);
        assert_eq!(summary.resource_count, 250);
        assert!(json.contains("resource-000"));
        bencher
            .with_inputs(|| summary.clone())
            .bench_values(|summary| black_box(domain_helm::encode(&summary)));
    }
}

mod helm_reconciliation_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn helm_reconciliation_json_encoding(bencher: Bencher) {
        let (summary, rows, unavailable, warnings) = domain_helm::reconciliation_assembly_inputs();
        let result = domain_helm::reconciliation_assemble(summary, rows, unavailable, warnings);
        let json = domain_helm::encode(&result);
        assert_eq!(result.resources.len(), 253);
        assert!(json.contains("resource-249"));
        bencher
            .with_inputs(|| result.clone())
            .bench_values(|result| black_box(domain_helm::encode(&result)));
    }
}
