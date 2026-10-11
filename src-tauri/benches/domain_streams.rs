use codspeed_divan_compat::{black_box, Bencher};
use kubecove_lib::commands::bench_support::domain_catalog::domain_streams;
use std::sync::LazyLock;

static LOG_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("benchmark log runtime")
});

mod bounded_log_line_decode_and_drain {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn bounded_log_line_decode_and_drain(bencher: Bencher) {
        let readers = domain_streams::log_reader_cases();
        let expected = domain_streams::decode_log_reader_cases(readers, &LOG_RUNTIME);
        assert_eq!(expected.len(), 6);
        assert_eq!(expected[0].as_ref().expect("valid text lines").len(), 3);
        assert_eq!(
            expected[1].as_ref().expect("exact-bound lines")[0].len(),
            256 * 1024
        );
        let oversized = expected[2].as_ref().expect("oversized line drains");
        assert_eq!(oversized.len(), 2);
        assert!(oversized[0].ends_with(" [truncated at 256 KiB]"));
        assert_eq!(oversized[1], "next");
        assert!(expected[3]
            .as_ref()
            .expect("truncated multibyte line is retained")
            .first()
            .expect("one split line")
            .ends_with(" [truncated at 256 KiB]"));
        assert_eq!(expected[4], Err(std::io::ErrorKind::InvalidData));
        assert!(expected[5].as_ref().expect("empty reader").is_empty());

        bencher
            .with_inputs(domain_streams::log_reader_cases)
            .bench_values(|readers| {
                black_box(domain_streams::decode_log_reader_cases(
                    readers,
                    &LOG_RUNTIME,
                ))
            });
    }
}

mod exec_incremental_utf8_decode {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn exec_incremental_utf8_decode(bencher: Bencher) {
        let chunks = domain_streams::exec_utf8_chunks();
        assert_eq!(
            chunks.iter().take(2).map(Vec::len).collect::<Vec<_>>(),
            [4096, 4096]
        );
        let output = domain_streams::decode_exec_utf8_chunks(&chunks);
        let text = output.concat();
        assert!(text.contains('€'));
        assert_eq!(text.matches('\u{FFFD}').count(), 2_048);
        assert!(text.ends_with("z\u{FFFD}"));

        bencher
            .with_inputs(|| chunks.clone())
            .bench_values(|chunks| black_box(domain_streams::decode_exec_utf8_chunks(&chunks)));
    }
}

mod aggregate_log_source_enumeration {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn aggregate_log_source_enumeration(bencher: Bencher) {
        let pods = domain_streams::aggregate_log_pods();
        let (active, total) = domain_streams::aggregate_source_counts(&pods);
        assert_eq!((active, total), (50, 75));
        assert!(pods.iter().any(|pod| {
            pod.metadata.name.as_deref() == Some("api-00")
                && pod
                    .spec
                    .as_ref()
                    .is_some_and(|spec| spec.containers.len() == 2)
        }));
        assert_eq!(
            domain_streams::first_aggregate_source(&pods),
            Some(("api-00".to_string(), "app".to_string()))
        );

        bencher
            .with_inputs(domain_streams::aggregate_log_pods)
            .bench_values(|pods| black_box(domain_streams::aggregate_source_counts(&pods)));
    }
}

mod bounded_log_messages_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn bounded_log_messages_json_encoding(bencher: Bencher) {
        let decoded = domain_streams::decode_log_reader_cases(
            domain_streams::log_reader_cases(),
            &LOG_RUNTIME,
        );
        let messages = domain_streams::log_messages(&decoded);
        assert_eq!(messages.len(), 9);
        let encoded_bytes = domain_streams::encode_each_message(&messages);
        assert!(encoded_bytes > 256 * 1024);
        bencher
            .with_inputs(|| messages.clone())
            .bench_values(|messages| black_box(domain_streams::encode_each_message(&messages)));
    }
}

mod exec_output_messages_json_encoding {
    use super::*;

    #[codspeed_divan_compat::bench]
    fn exec_output_messages_json_encoding(bencher: Bencher) {
        let outputs = domain_streams::decode_exec_utf8_chunks(&domain_streams::exec_utf8_chunks());
        let messages = domain_streams::exec_messages(&outputs);
        assert_eq!(messages.len(), outputs.len());
        assert!(domain_streams::encode_each_message(&messages) > outputs.concat().len());
        bencher
            .with_inputs(|| messages.clone())
            .bench_values(|messages| black_box(domain_streams::encode_each_message(&messages)));
    }
}
