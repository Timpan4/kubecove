use codspeed_divan_compat::{black_box, Bencher};
use kubecove_lib::commands::bench_support::domain_catalog::domain_flux;

mod flux_list_projection {
    use super::*;

    // R35 and folded R34: installed API matching once per preparation plus
    // complete summaries across Source, Kustomization, and HelmRelease batches.
    #[codspeed_divan_compat::bench]
    fn flux_list_projection(bencher: Bencher) {
        let discovery = domain_flux::discovery_resources();
        let batches = domain_flux::flux_object_batches();
        assert_eq!(batches.len(), 3);
        assert!(batches.iter().all(|batch| batch.objects.len() == 250));
        let expected = domain_flux::project_list(&discovery, &batches);
        assert_eq!(expected.summaries.len(), 750);
        for (group, kind) in [
            ("source.toolkit.fluxcd.io", "GitRepository"),
            ("source.toolkit.fluxcd.io", "OCIRepository"),
            ("source.toolkit.fluxcd.io", "HelmRepository"),
            ("source.toolkit.fluxcd.io", "HelmChart"),
            ("source.toolkit.fluxcd.io", "Bucket"),
            ("kustomize.toolkit.fluxcd.io", "Kustomization"),
            ("helm.toolkit.fluxcd.io", "HelmRelease"),
            ("notification.toolkit.fluxcd.io", "Provider"),
            ("notification.toolkit.fluxcd.io", "Alert"),
            ("notification.toolkit.fluxcd.io", "Receiver"),
            ("image.toolkit.fluxcd.io", "ImageRepository"),
            ("image.toolkit.fluxcd.io", "ImagePolicy"),
            ("image.toolkit.fluxcd.io", "ImageUpdateAutomation"),
        ] {
            assert!(expected
                .installed_kinds
                .iter()
                .any(|installed| installed.group == group && installed.kind == kind));
        }
        assert_eq!(expected.installed_kinds.len(), 13);
        let git_kind = expected
            .installed_kinds
            .iter()
            .find(|kind| kind.kind == "GitRepository")
            .expect("GitRepository fixture is discovered");
        assert_eq!(git_kind.api_version, "source.toolkit.fluxcd.io/v1beta2");
        let bounded_inventory = &expected.summaries[0];
        assert_eq!(bounded_inventory.ready_status.as_deref(), Some("False"));
        assert_eq!(
            bounded_inventory.message.as_deref(),
            Some("GitRepository-000 status")
        );
        assert_eq!(
            bounded_inventory.inventory[0].version.as_deref(),
            Some("v1")
        );
        assert_eq!(
            bounded_inventory.inventory[2].version.as_deref(),
            Some("v1")
        );
        assert_eq!(bounded_inventory.suspended, Some(true));
        assert_eq!(bounded_inventory.source_name.as_deref(), Some("payments"));
        assert_eq!(bounded_inventory.inventory.len(), 2_045);
        assert!(bounded_inventory.inventory_partial);
        let reason_and_artifact_fallback = &expected.summaries[1];
        assert_eq!(
            reason_and_artifact_fallback.ready_status.as_deref(),
            Some("SourceArtifactReady")
        );
        assert_eq!(
            reason_and_artifact_fallback
                .last_applied_revision
                .as_deref(),
            Some("artifact-revision-001")
        );

        bencher
            .with_inputs(|| (discovery.clone(), batches.clone()))
            .bench_values(|(discovery, batches)| {
                black_box(domain_flux::project_list(&discovery, &batches))
            });
    }
}

mod flux_resource_list_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn flux_resource_list_json_encoding(bencher: Bencher) {
        let discovery = domain_flux::discovery_resources();
        let batches = domain_flux::flux_object_batches();
        let output = domain_flux::project_list(&discovery, &batches).summaries;
        let json = domain_flux::encode(&output);
        assert_eq!(output.len(), 750);
        assert!(json.contains("gitrepository-249"));
        assert!(json.contains("kustomization-249"));
        assert!(json.contains("helmrelease-249"));
        bencher
            .with_inputs(|| output.clone())
            .bench_values(|summaries| black_box(domain_flux::encode(&summaries)));
    }
}
