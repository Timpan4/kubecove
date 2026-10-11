use codspeed_divan_compat::{black_box, Bencher};
use kubecove_lib::commands::bench_support::domain_catalog::domain_usage;
use kubecove_lib::commands::bench_support::domain_catalog::domain_usage::UsageProcessPlatform;
use kubecove_lib::models::AppUsageMetrics;

fn assert_attribution(metrics: &AppUsageMetrics, expected_count: usize) {
    assert_eq!(metrics.process_count, expected_count);
    assert_eq!(metrics.cpu_percent, expected_count as f32 * 0.09375);
    assert!(metrics.breakdown.iter().any(|row| row.label == "KubeCove"));
    assert_eq!(
        metrics
            .breakdown
            .iter()
            .map(|row| row.memory_bytes)
            .sum::<u64>(),
        metrics.memory_bytes
    );
    assert!(metrics
        .breakdown
        .iter()
        .flat_map(|group| group.children.windows(2))
        .all(|children| children[0].memory_bytes >= children[1].memory_bytes));
}

mod usage_process_attribution_native {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn usage_process_attribution_native(bencher: Bencher) {
        let inputs = domain_usage::process_inputs();
        let expected =
            domain_usage::attribute_processes(inputs.clone(), UsageProcessPlatform::Native);
        assert_attribution(&expected, 221);
        assert_eq!(
            expected.breakdown[0].children.len()
                + expected.breakdown[1].children.len()
                + expected
                    .breakdown
                    .get(2)
                    .map_or(0, |row| row.children.len()),
            220
        );

        bencher
            .with_inputs(|| inputs.clone())
            .bench_values(|inputs| {
                black_box(domain_usage::attribute_processes(
                    inputs,
                    UsageProcessPlatform::Native,
                ))
            });
    }
}

mod usage_process_attribution_macos_orphans {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn usage_process_attribution_macos_orphans(bencher: Bencher) {
        let inputs = domain_usage::process_inputs();
        let expected =
            domain_usage::attribute_processes(inputs.clone(), UsageProcessPlatform::MacOs);
        assert_attribution(&expected, 225);
        let webview = expected
            .breakdown
            .iter()
            .find(|group| group.label == "WebView")
            .expect("WebKit process group");
        assert!(webview.process_count >= 4);
        assert!(webview
            .children
            .iter()
            .any(|child| child.label == "Renderer"));

        bencher
            .with_inputs(|| inputs.clone())
            .bench_values(|inputs| {
                black_box(domain_usage::attribute_processes(
                    inputs,
                    UsageProcessPlatform::MacOs,
                ))
            });
    }
}

mod usage_process_attribution_macos_ambiguous_cohort {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn usage_process_attribution_macos_ambiguous_cohort(bencher: Bencher) {
        let inputs = domain_usage::ambiguous_macos_cohort_inputs();
        let expected =
            domain_usage::attribute_processes(inputs.clone(), UsageProcessPlatform::MacOs);
        assert_attribution(&expected, 221);
        let native = domain_usage::attribute_processes(
            domain_usage::process_inputs(),
            UsageProcessPlatform::Native,
        );
        assert_eq!(expected.memory_bytes, native.memory_bytes);

        bencher
            .with_inputs(|| inputs.clone())
            .bench_values(|inputs| {
                black_box(domain_usage::attribute_processes(
                    inputs,
                    UsageProcessPlatform::MacOs,
                ))
            });
    }
}

mod usage_process_attribution_windows_webview {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn usage_process_attribution_windows_webview(bencher: Bencher) {
        let inputs = domain_usage::process_inputs();
        let expected =
            domain_usage::attribute_processes(inputs.clone(), UsageProcessPlatform::Windows);
        assert_attribution(&expected, 223);
        let webview = expected
            .breakdown
            .iter()
            .find(|group| group.label == "WebView")
            .expect("WebView2 process group");
        assert!(webview
            .children
            .iter()
            .any(|child| child.label == "Browser process"));
        assert!(webview
            .children
            .iter()
            .any(|child| child.label == "Renderer"));

        bencher
            .with_inputs(|| inputs.clone())
            .bench_values(|inputs| {
                black_box(domain_usage::attribute_processes(
                    inputs,
                    UsageProcessPlatform::Windows,
                ))
            });
    }
}

mod usage_process_attribution_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn usage_process_attribution_json_encoding(bencher: Bencher) {
        let metrics = domain_usage::attribute_processes(
            domain_usage::process_inputs(),
            UsageProcessPlatform::MacOs,
        );
        let json = domain_usage::encode(&metrics);
        assert!(json.contains("sampledAt"));
        assert!(json.contains("Network service"));
        bencher
            .with_inputs(|| metrics.clone())
            .bench_values(|metrics| black_box(domain_usage::encode(&metrics)));
    }
}
