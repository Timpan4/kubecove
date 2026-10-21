use futures_util::io::{BufReader, Cursor};
use k8s_openapi::api::core::v1::{Container, EphemeralContainer, Pod, PodSpec, PodStatus};
use kube::api::ObjectMeta;

use crate::{
    commands::{
        pod_exec::benchmark_decode_exec_chunks,
        streams::{benchmark_log_sources_from_pods, benchmark_read_log_line, MAX_LOG_LINE_BYTES},
    },
    models::{LogLineSource, PodExecSessionMessage, StreamMessage},
};

pub type LogReader = BufReader<Cursor<Vec<u8>>>;

pub fn log_reader_cases() -> Vec<LogReader> {
    let simple = b"short-line\ncrlf-line\r\nlast-without-newline".to_vec();
    let mut exact = vec![b'e'; MAX_LOG_LINE_BYTES];
    exact.extend_from_slice(b"\r\nnext\n");
    let mut oversized = vec![b'o'; MAX_LOG_LINE_BYTES + 1];
    oversized.extend_from_slice(b"\nnext\n");
    let mut split_utf8 = vec![b'x'; MAX_LOG_LINE_BYTES - 1];
    split_utf8.extend_from_slice("é".as_bytes());
    split_utf8.push(b'\n');
    let invalid = vec![0xff, b'\n', b'n', b'e', b'x', b't', b'\n'];
    vec![simple, exact, oversized, split_utf8, invalid, Vec::new()]
        .into_iter()
        .map(|bytes| BufReader::new(Cursor::new(bytes)))
        .collect()
}

pub fn decode_log_reader_cases(
    mut readers: Vec<LogReader>,
    runtime: &tokio::runtime::Runtime,
) -> Vec<Result<Vec<String>, std::io::ErrorKind>> {
    runtime.block_on(async move {
        let mut cases = Vec::with_capacity(readers.len());
        for reader in &mut readers {
            let mut lines = Vec::new();
            loop {
                match benchmark_read_log_line(reader).await {
                    Ok(Some(line)) => lines.push(line),
                    Ok(None) => {
                        cases.push(Ok(lines));
                        break;
                    }
                    Err(error) => {
                        cases.push(Err(error.kind()));
                        break;
                    }
                }
            }
        }
        cases
    })
}

pub fn exec_utf8_chunks() -> Vec<Vec<u8>> {
    let mut first = vec![b'a'; crate::commands::pod_exec::EXEC_OUTPUT_CHUNK_BYTES - 1];
    first.push(0xe2);
    let mut second = vec![0x82, 0xac];
    for _ in 0..(crate::commands::pod_exec::EXEC_OUTPUT_CHUNK_BYTES - 2) / 2 {
        second.extend_from_slice(b"x\xff");
    }
    let third = vec![b'z', 0xf0, 0x9f];
    vec![first, second, third]
}

pub fn decode_exec_utf8_chunks(chunks: &[Vec<u8>]) -> Vec<String> {
    benchmark_decode_exec_chunks(chunks)
}

pub fn aggregate_log_pods() -> Vec<Pod> {
    let mut pods = (0_usize..60)
        .map(|index| {
            let mut containers = vec![Container {
                name: " app ".to_string(),
                ..Default::default()
            }];
            if index.is_multiple_of(4) {
                containers.push(Container {
                    name: " metrics ".to_string(),
                    ..Default::default()
                });
            }
            Pod {
                metadata: ObjectMeta {
                    name: Some(format!("api-{index:02}")),
                    ..Default::default()
                },
                spec: Some(PodSpec {
                    containers,
                    init_containers: Some(vec![Container {
                        name: "init-only".to_string(),
                        ..Default::default()
                    }]),
                    ephemeral_containers: Some(vec![EphemeralContainer {
                        name: "debug-only".to_string(),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();
    let deleting = Pod {
        metadata: ObjectMeta {
            name: Some("api-deleting".to_string()),
            deletion_timestamp: Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(
                k8s_openapi::jiff::Timestamp::from_second(0).expect("fixture timestamp"),
            )),
            ..Default::default()
        },
        spec: Some(PodSpec {
            containers: vec![Container {
                name: "app".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    pods.push(deleting);
    pods.push(Pod {
        metadata: ObjectMeta::default(),
        status: Some(PodStatus::default()),
        ..Default::default()
    });
    pods.reverse();
    pods
}

pub fn aggregate_source_counts(pods: &[Pod]) -> (usize, usize) {
    let (sources, total) = benchmark_log_sources_from_pods(pods);
    std::hint::black_box(&sources);
    (sources.len(), total)
}

pub fn first_aggregate_source(pods: &[Pod]) -> Option<(String, String)> {
    benchmark_log_sources_from_pods(pods)
        .0
        .first()
        .map(|source| (source.pod_name.clone(), source.container.clone()))
}

pub fn log_messages(cases: &[Result<Vec<String>, std::io::ErrorKind>]) -> Vec<StreamMessage> {
    cases
        .iter()
        .flat_map(|case| match case {
            Ok(lines) => lines
                .iter()
                .map(|line| StreamMessage::LogLines {
                    stream_id: "log-stream-17".to_string(),
                    lines: vec![line.clone()],
                    source: Some(LogLineSource {
                        pod_name: "api-17".to_string(),
                        container: Some("app".to_string()),
                    }),
                })
                .collect::<Vec<_>>(),
            Err(error) => vec![StreamMessage::Error {
                stream_id: "log-stream-17".to_string(),
                message: std::io::Error::from(*error).to_string(),
            }],
        })
        .collect()
}

pub fn exec_messages(outputs: &[String]) -> Vec<PodExecSessionMessage> {
    outputs
        .iter()
        .map(|data| PodExecSessionMessage::Output {
            session_id: "exec-session-1".to_string(),
            stream: "stdout".to_string(),
            data: data.clone(),
        })
        .collect()
}

pub fn encode_each_message<T: serde::Serialize>(messages: &[T]) -> usize {
    messages
        .iter()
        .map(|message| super::encode_complete_response(message).len())
        .sum()
}
