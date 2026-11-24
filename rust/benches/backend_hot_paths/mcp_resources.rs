use codspeed_divan_compat::{Bencher, black_box};
use kubecove_inspection::commands::resources::{
    benchmark_shared_scope_response, dynamic_details_from_fetched,
};
use kubecove_lib::{
    commands::bench_support::resource_catalog::{resource_cache_fixture, yaml_document_fixture},
    models::{ResourceSummary, YamlViewMode},
};
use kubecove_mcp::bench_support::{describe_resource_body, page_resource_rows};
use std::sync::{Arc, LazyLock};

static CACHED_ROWS: LazyLock<Arc<Vec<ResourceSummary>>> =
    LazyLock::new(|| Arc::new(resource_cache_fixture().expected_rows().to_vec()));

#[codspeed_divan_compat::bench]
fn mcp_warm_resource_scope_small_page_10000(bencher: Bencher) {
    let page = || {
        let scope =
            benchmark_shared_scope_response(vec![Arc::clone(&CACHED_ROWS)], &Default::default());
        page_resource_rows(&scope, Some(10), Some(20), |_| true).unwrap()
    };
    let output = page();
    assert_eq!(output["total"], 10_000);
    assert_eq!(output["offset"], 20);
    assert_eq!(output["nextOffset"], 30);
    assert_eq!(output["resources"].as_array().unwrap().len(), 10);
    bencher.bench(|| black_box(page()));
}

#[codspeed_divan_compat::bench]
fn mcp_describe_dynamic_resource(bencher: Bencher) {
    let fixture = yaml_document_fixture();
    let project = |object| {
        let details = dynamic_details_from_fetched(
            "catalog-cluster",
            &fixture.dynamic_kind,
            object,
            YamlViewMode::Kubectl,
        )
        .unwrap();
        describe_resource_body("catalog-cluster".into(), details)
    };
    let output = project(fixture.dynamic_object.clone());
    assert!(output["fields"]["spec"].is_object());
    assert!(
        !output["fields"]
            .as_object()
            .unwrap()
            .contains_key("metadata")
    );
    bencher
        .with_inputs(|| fixture.dynamic_object.clone())
        .bench_values(|object| black_box(project(object)));
}
