use kubecove_lib::commands::bench_support::{cache_diagnostic_report, cached_topology_fixture};

fn main() {
    // Reuses the existing backend benchmark's 500-app workload, without API calls.
    let store = cached_topology_fixture(500);
    println!(
        "{}",
        serde_json::to_string_pretty(&cache_diagnostic_report(&store))
            .expect("redacted cache report")
    );
}
