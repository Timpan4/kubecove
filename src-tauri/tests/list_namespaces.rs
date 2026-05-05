use k8s_manager_lib::commands::namespaces_summary_from;
use k8s_openapi::api::core::v1::{Namespace, NamespaceStatus};
use k8s_openapi::jiff::Timestamp;

fn make_namespace(name: &str, phase: Option<&str>) -> Namespace {
    let mut ns = Namespace::default();
    ns.metadata.name = Some(name.to_owned());
    ns.metadata.creation_timestamp = Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(
        Timestamp::now(),
    ));
    if let Some(p) = phase {
        ns.status = Some(NamespaceStatus {
            phase: Some(p.to_owned()),
            conditions: None,
        });
    }
    ns
}

#[test]
fn test_namespaces_summary_from_active() {
    let ns = make_namespace("default", Some("Active"));
    let summaries = namespaces_summary_from(&[ns]);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].name, "default");
    assert_eq!(summaries[0].status.as_deref(), Some("Active"));
    assert!(summaries[0].age.is_some());
}

#[test]
fn test_namespaces_summary_from_terminating() {
    let ns = make_namespace("kube-system", Some("Terminating"));
    let summaries = namespaces_summary_from(&[ns]);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].name, "kube-system");
    assert_eq!(summaries[0].status.as_deref(), Some("Terminating"));
    assert!(summaries[0].age.is_some());
}

#[test]
fn test_namespaces_summary_omits_nil_phase() {
    let ns = make_namespace("no-phase", None);
    let summaries = namespaces_summary_from(&[ns]);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].name, "no-phase");
    assert!(summaries[0].status.is_none());
}

#[test]
fn test_namespaces_summary_multiple() {
    let ns1 = make_namespace("default", Some("Active"));
    let ns2 = make_namespace("kube-system", Some("Active"));
    let summaries = namespaces_summary_from(&[ns1, ns2]);
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].name, "default");
    assert_eq!(summaries[1].name, "kube-system");
}