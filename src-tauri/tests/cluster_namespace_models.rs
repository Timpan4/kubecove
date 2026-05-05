use k8s_manager_lib::models::{ClusterContext, NamespaceSummary};
use serde_json::json;

#[test]
fn optional_fields_are_omitted_from_serialized_contracts() {
    let context = ClusterContext {
        name: "kind-dev".to_string(),
        cluster: None,
        user: None,
        namespace: None,
    };
    let namespace = NamespaceSummary {
        name: "default".to_string(),
        status: None,
        age: None,
    };

    assert_eq!(
        serde_json::to_value(context).expect("cluster context should serialize"),
        json!({ "name": "kind-dev" })
    );
    assert_eq!(
        serde_json::to_value(namespace).expect("namespace summary should serialize"),
        json!({ "name": "default" })
    );
}
