use codspeed_divan_compat::{Bencher, black_box};
use kubecove_inspection::commands::resources::{
    benchmark_shared_scope_response, dynamic_details_from_fetched,
};
use kubecove_lib::{
    commands::bench_support::resource_catalog::{resource_cache_fixture, yaml_document_fixture},
    models::{HealthAssessmentState, ResourceSummary, YamlViewMode},
};
use kubecove_mcp::bench_support::{
    HealthFilter, describe_resource_body, page_resource_rows, resource_row_matches,
};
use serde_json::{Map, Value};
use std::sync::{Arc, LazyLock};

static CACHED_ROWS: LazyLock<Arc<Vec<ResourceSummary>>> =
    LazyLock::new(|| Arc::new(resource_cache_fixture().expected_rows().to_vec()));

const FILTERED_NAME: &str = "ledger-1";

// Expected pages are built from the fixture outside the timed section, so a wrong
// sort, filter, or offset changes the asserted rows.
fn sorted_rows<'a>(rows: impl Iterator<Item = &'a ResourceSummary>) -> Vec<&'a ResourceSummary> {
    let mut rows: Vec<_> = rows.collect();
    rows.sort_by(|left, right| (&left.namespace, &left.name).cmp(&(&right.namespace, &right.name)));
    rows
}

fn expected_identities<'a>(rows: &[&'a ResourceSummary]) -> Vec<(Option<&'a str>, &'a str)> {
    rows.iter()
        .map(|row| (row.namespace.as_deref(), row.name.as_str()))
        .collect()
}

fn output_identities(output: &Map<String, Value>) -> Vec<(Option<&str>, &str)> {
    output["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["namespace"].as_str(), row["name"].as_str().unwrap()))
        .collect()
}

#[codspeed_divan_compat::bench]
fn mcp_warm_resource_scope_small_page_10000(bencher: Bencher) {
    let page = || {
        let scope =
            benchmark_shared_scope_response(vec![Arc::clone(&CACHED_ROWS)], &Default::default());
        page_resource_rows(scope.rows(), Some(10), Some(20), |_| true).unwrap()
    };
    let output = page();
    let expected = sorted_rows(CACHED_ROWS.iter());
    assert_eq!(output["total"], 10_000);
    assert_eq!(output["offset"], 20);
    assert_eq!(output["nextOffset"], 30);
    assert_eq!(
        output_identities(&output),
        expected_identities(&expected[20..30])
    );
    bencher.bench(|| black_box(page()));
}

#[codspeed_divan_compat::bench]
fn mcp_warm_resource_scope_filtered_small_page_10000(bencher: Bencher) {
    let page = || {
        let scope =
            benchmark_shared_scope_response(vec![Arc::clone(&CACHED_ROWS)], &Default::default());
        page_resource_rows(scope.rows(), Some(10), Some(20), |row| {
            resource_row_matches(Some(FILTERED_NAME), HealthFilter::Unhealthy, row)
        })
        .unwrap()
    };
    let expected = sorted_rows(CACHED_ROWS.iter().filter(|row| {
        row.name.contains(FILTERED_NAME)
            && matches!(
                row.health_assessment.state,
                HealthAssessmentState::Degraded
                    | HealthAssessmentState::NeedsAttention
                    | HealthAssessmentState::Unknown
            )
    }));
    assert!(
        expected.len() > 30,
        "fixture needs more than two pages of unhealthy {FILTERED_NAME} rows"
    );
    let output = page();
    assert_eq!(
        output["total"].as_u64(),
        Some(u64::try_from(expected.len()).unwrap())
    );
    assert_eq!(output["offset"], 20);
    assert_eq!(output["nextOffset"], 30);
    assert_eq!(
        output_identities(&output),
        expected_identities(&expected[20..30])
    );
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
