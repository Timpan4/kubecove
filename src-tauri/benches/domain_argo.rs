use codspeed_divan_compat::{black_box, Bencher};
use kubecove_lib::commands::bench_support::domain_catalog::domain_argo;
use kubecove_lib::models::HealthAssessmentState;

mod argo_list_projection {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_list_projection(bencher: Bencher) {
        let objects = domain_argo::list_objects();
        let expected = domain_argo::project_list_summaries(&objects.0, &objects.1, &objects.2);
        assert_eq!(expected.applications.len(), 250);
        assert_eq!(expected.application_sets.len(), 250);
        assert_eq!(expected.projects.len(), 250);
        let application = &expected.applications[17];
        assert_eq!(application.sources.len(), 4);
        // List fixtures carry four status resources; inspector fixtures carry 250.
        assert_eq!(application.tracked_resource_count, Some(4));
        assert_eq!(application.resource_namespaces.len(), 4);
        assert_eq!(application.source_revision.as_deref(), Some("2 revisions"));
        assert_eq!(application.operation_phase.as_deref(), Some("Running"));
        assert_eq!(application.health_status.as_deref(), Some("Healthy"));
        assert_eq!(
            application.health_assessment.state,
            HealthAssessmentState::Healthy
        );
        assert_eq!(
            application.sources[0].resolved_revision.as_deref(),
            Some("main")
        );
        assert_eq!(
            application.sources[1].resolved_revision.as_deref(),
            Some("release")
        );
        assert_eq!(
            application.sources[0].reference.as_deref(),
            Some("source-0")
        );
        let head_fallback = &expected.applications[249].sources[0];
        assert_eq!(head_fallback.target_revision.as_deref(), Some("HEAD"));
        assert_eq!(head_fallback.resolved_revision.as_deref(), Some("main"));
        assert_eq!(
            expected.application_sets[249].health_status.as_deref(),
            Some("Healthy")
        );
        assert_eq!(
            expected.application_sets[249].health_assessment.state,
            HealthAssessmentState::Healthy
        );
        assert_eq!(expected.projects[249].status.as_deref(), Some("ValidSpec"));

        bencher
            .with_inputs(|| (objects.0.clone(), objects.1.clone(), objects.2.clone()))
            .bench_values(|(applications, appsets, projects)| {
                black_box(domain_argo::project_list_summaries(
                    &applications,
                    &appsets,
                    &projects,
                ))
            });
    }
}

mod argo_kubernetes_inspector {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_kubernetes_inspector(bencher: Bencher) {
        let object = domain_argo::kubernetes_inspector_input();
        let application = domain_argo::application_ref("checkout-17");
        let expected = domain_argo::kubernetes_inspector(object.clone(), &application)
            .expect("Kubernetes inspector fixture is valid");
        assert_eq!(expected.resources.len(), 250);
        assert_eq!(expected.history.len(), 250);
        assert_eq!(expected.conditions.len(), 250);
        assert_eq!(expected.comparisons.len(), 250);
        assert_eq!(expected.application.context.as_deref(), Some("benchmark"));
        assert_eq!(expected.provenance, "kubernetes-status-no-diff");
        assert_eq!(expected.history.first().and_then(|row| row.id), Some(0));
        assert_eq!(expected.history.last().and_then(|row| row.id), Some(249));
        assert_eq!(
            expected.resources[249].name.as_deref(),
            Some("deployment-249")
        );
        assert_eq!(
            object.data["status"]["resources"][0]["name"],
            "deployment-249"
        );
        assert_eq!(object.data["status"]["resources"][0]["group"], "example.io");
        assert_eq!(object.data["status"]["resources"][1]["kind"], "StatefulSet");
        assert_eq!(
            object.data["status"]["resources"][2]["namespace"],
            "other-namespace"
        );
        assert_eq!(expected.comparisons[249].exact, Some(false));

        bencher
            .with_inputs(|| object.clone())
            .bench_values(|object| {
                black_box(
                    domain_argo::kubernetes_inspector(object, &application)
                        .expect("fixture remains valid"),
                )
            });
    }
}

mod argo_connected_inspector {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_connected_inspector(bencher: Bencher) {
        let (application, response, managed) = domain_argo::connected_inspector_inputs();
        let expected = domain_argo::connected_inspector(
            application.clone(),
            response.clone(),
            managed.clone(),
        );
        assert_eq!(expected.resources.len(), 250);
        assert_eq!(expected.history.len(), 250);
        assert_eq!(expected.conditions.len(), 250);
        assert_eq!(expected.comparisons.len(), 250);
        assert!(expected.connected);
        assert_eq!(expected.provenance, "argocd-api");
        assert_eq!(expected.history.first().and_then(|row| row.id), Some(0));
        assert_eq!(expected.history.last().and_then(|row| row.id), Some(249));
        assert_eq!(expected.comparisons[249].modified, Some(true));
        assert!(expected.comparisons[249].target_state.is_some());
        assert_eq!(
            expected.comparisons[249]
                .target_state
                .as_ref()
                .and_then(|state| state.pointer("/data/password"))
                .and_then(serde_json::Value::as_str),
            Some("[REDACTED]")
        );
        let managed_items = managed["items"]
            .as_array()
            .expect("connected managed resource fixture");
        assert_eq!(
            managed_items
                .iter()
                .filter(|item| item["name"] == "late-secret")
                .count(),
            4
        );
        assert_eq!(managed_items[0]["group"], "example.io");
        assert_eq!(managed_items[1]["kind"], "ConfigMap");
        assert_eq!(managed_items[2]["namespace"], "other-namespace");
        let serialized = serde_json::to_string(&expected).expect("inspector serializes");
        for secret in [
            "target-secret",
            "target-token",
            "live-secret",
            "normalized-secret",
        ] {
            assert!(
                !serialized.contains(secret),
                "connected inspector leaked a fixture credential"
            );
        }

        bencher
            .with_inputs(|| (response.clone(), managed.clone()))
            .bench_values(|(response, managed)| {
                black_box(domain_argo::connected_inspector(
                    application.clone(),
                    response,
                    managed,
                ))
            });
    }
}

mod argo_selected_comparison_kubernetes {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_selected_comparison_kubernetes(bencher: Bencher) {
        let application = domain_argo::kubernetes_inspector_input();
        let target = domain_argo::selected_resource_in_namespace(
            "deployment-249",
            "Deployment",
            "tenant-24",
        );
        let expected = domain_argo::selected_kubernetes_comparison(&application, &target)
            .expect("late Kubernetes managed resource matches");
        assert_eq!(expected.resource.name.as_deref(), Some("deployment-249"));
        assert_eq!(expected.resource.group.as_deref(), Some("apps"));
        assert_eq!(expected.resource.version.as_deref(), Some("v1"));
        assert_eq!(expected.resource.kind.as_deref(), Some("Deployment"));
        assert_eq!(expected.resource.namespace.as_deref(), Some("tenant-24"));
        assert_eq!(expected.exact, Some(false));
        let missing = domain_argo::selected_resource_in_namespace(
            "absent-resource",
            "Deployment",
            "tenant-24",
        );
        assert!(domain_argo::selected_kubernetes_comparison(&application, &missing).is_err());

        bencher
            .with_inputs(|| application.clone())
            .bench_values(|application| {
                black_box(
                    domain_argo::selected_kubernetes_comparison(&application, &target)
                        .expect("fixture target remains present"),
                )
            });
    }
}

mod argo_selected_comparison_connected {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_selected_comparison_connected(bencher: Bencher) {
        let (_, _, managed) = domain_argo::connected_inspector_inputs();
        let target = domain_argo::selected_resource("late-secret", "Secret");
        let expected = domain_argo::selected_connected_comparison(managed.clone(), &target)
            .expect("late connected Secret matches");
        assert_eq!(expected.resource.name.as_deref(), Some("late-secret"));
        assert_eq!(expected.resource.group.as_deref(), Some(""));
        assert_eq!(expected.resource.version.as_deref(), Some("v1"));
        assert_eq!(expected.resource.kind.as_deref(), Some("Secret"));
        assert_eq!(expected.resource.namespace.as_deref(), Some("payments"));
        assert_eq!(expected.exact, Some(true));
        assert_eq!(
            expected
                .target_state
                .as_ref()
                .and_then(|state| state.pointer("/data/password"))
                .and_then(serde_json::Value::as_str),
            Some("[REDACTED]")
        );
        let serialized = serde_json::to_string(&expected).expect("comparison serializes");
        for secret in [
            "target-secret",
            "target-token",
            "live-secret",
            "normalized-secret",
        ] {
            assert!(
                !serialized.contains(secret),
                "connected comparison leaked a fixture credential"
            );
        }
        let missing = domain_argo::selected_resource("absent-resource", "Secret");
        assert!(domain_argo::selected_connected_comparison(managed.clone(), &missing).is_err());

        bencher
            .with_inputs(|| managed.clone())
            .bench_values(|managed| {
                black_box(
                    domain_argo::selected_connected_comparison(managed, &target)
                        .expect("fixture target remains present"),
                )
            });
    }
}

mod argo_kubernetes_identity_revalidation {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_kubernetes_identity_revalidation(bencher: Bencher) {
        let current = domain_argo::kubernetes_inspector_input();
        let expected = domain_argo::application_ref("checkout-17");
        assert!(domain_argo::identity_revalidation(&current, &expected).is_ok());
        for mutate in ["name", "namespace", "project", "uid", "resourceVersion"] {
            let mut stale = expected.clone();
            match mutate {
                "name" => stale.name.push_str("-stale"),
                "namespace" => stale.namespace = Some("other-namespace".to_string()),
                "project" => stale.project = Some("other-project".to_string()),
                "uid" => stale.uid = Some("other-uid".to_string()),
                "resourceVersion" => stale.resource_version = Some("other-version".to_string()),
                _ => unreachable!(),
            }
            assert!(domain_argo::identity_revalidation(&current, &stale).is_err());
        }

        bencher
            .with_inputs(|| current.clone())
            .bench_values(|current| {
                black_box(domain_argo::identity_revalidation(&current, &expected))
            });
    }
}

mod argo_application_list_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_application_list_json_encoding(bencher: Bencher) {
        let objects = domain_argo::list_objects();
        let output = domain_argo::project_list_summaries(&objects.0, &objects.1, &objects.2);
        let json = domain_argo::encode(&output.applications);
        assert!(json.contains("checkout-249"));
        assert!(json.contains("destinationNamespace"));
        bencher
            .with_inputs(|| output.applications.clone())
            .bench_values(|summaries| black_box(domain_argo::encode(&summaries)));
    }
}

mod argo_appset_list_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_appset_list_json_encoding(bencher: Bencher) {
        let objects = domain_argo::list_objects();
        let output = domain_argo::project_list_summaries(&objects.0, &objects.1, &objects.2);
        let json = domain_argo::encode(&output.application_sets);
        assert!(json.contains("tenant-apps-249"));
        bencher
            .with_inputs(|| output.application_sets.clone())
            .bench_values(|summaries| black_box(domain_argo::encode(&summaries)));
    }
}

mod argo_project_list_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_project_list_json_encoding(bencher: Bencher) {
        let objects = domain_argo::list_objects();
        let output = domain_argo::project_list_summaries(&objects.0, &objects.1, &objects.2);
        let json = domain_argo::encode(&output.projects);
        assert!(json.contains("project-249"));
        bencher
            .with_inputs(|| output.projects.clone())
            .bench_values(|summaries| black_box(domain_argo::encode(&summaries)));
    }
}

mod argo_kubernetes_inspector_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_kubernetes_inspector_json_encoding(bencher: Bencher) {
        let object = domain_argo::kubernetes_inspector_input();
        let output =
            domain_argo::kubernetes_inspector(object, &domain_argo::application_ref("checkout-17"))
                .expect("Kubernetes inspector fixture is valid");
        let json = domain_argo::encode(&output);
        assert!(json.contains("deployment-249"));
        bencher
            .with_inputs(|| output.clone())
            .bench_values(|inspector| black_box(domain_argo::encode(&inspector)));
    }
}

mod argo_connected_inspector_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_connected_inspector_json_encoding(bencher: Bencher) {
        let (application, response, managed) = domain_argo::connected_inspector_inputs();
        let output = domain_argo::connected_inspector(application, response, managed);
        let json = domain_argo::encode(&output);
        assert!(json.contains("deployment-249"));
        assert!(!json.contains("live-secret"));
        bencher
            .with_inputs(|| output.clone())
            .bench_values(|inspector| black_box(domain_argo::encode(&inspector)));
    }
}

mod argo_selected_kubernetes_comparison_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_selected_kubernetes_comparison_json_encoding(bencher: Bencher) {
        let application = domain_argo::kubernetes_inspector_input();
        let target = domain_argo::selected_resource_in_namespace(
            "deployment-249",
            "Deployment",
            "tenant-24",
        );
        let output = domain_argo::selected_kubernetes_comparison(&application, &target)
            .expect("late Kubernetes managed resource matches");
        let json = domain_argo::encode(&output);
        assert!(json.contains("deployment-249"));
        assert!(json.contains("kubernetes-status-no-diff"));
        bencher
            .with_inputs(|| output.clone())
            .bench_values(|comparison| black_box(domain_argo::encode(&comparison)));
    }
}

mod argo_selected_comparison_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn argo_selected_comparison_json_encoding(bencher: Bencher) {
        let (_, _, managed) = domain_argo::connected_inspector_inputs();
        let output = domain_argo::selected_connected_comparison(
            managed,
            &domain_argo::selected_resource("late-secret", "Secret"),
        )
        .expect("late connected Secret matches");
        let json = domain_argo::encode(&output);
        assert!(json.contains("late-secret"));
        assert!(!json.contains("live-secret"));
        bencher
            .with_inputs(|| output.clone())
            .bench_values(|comparison| black_box(domain_argo::encode(&comparison)));
    }
}
