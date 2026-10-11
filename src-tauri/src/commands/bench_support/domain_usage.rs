use crate::{
    commands::usage::{benchmark_usage_process_attribution, BenchUsagePlatform, BenchUsageProcess},
    models::AppUsageMetrics,
};
use std::collections::HashMap;

#[derive(Clone)]
pub struct UsageProcessInputs {
    processes: HashMap<u32, BenchUsageProcess>,
    root_pid: u32,
    windows_host_names: Vec<String>,
    cpu_count: usize,
}

#[derive(Clone, Copy)]
pub enum UsageProcessPlatform {
    Native,
    MacOs,
    Windows,
}

pub fn process_inputs() -> UsageProcessInputs {
    const ROOT_PID: u32 = 10_000;
    const ROOT_START: u64 = 1_000;
    // Keep the deterministic table at the catalog's 250-row production-scale fixture.
    const PROCESS_TABLE_ROWS: usize = 250;

    let mut processes = Vec::with_capacity(PROCESS_TABLE_ROWS);
    processes.push(process(
        ROOT_PID,
        None,
        ROOT_START,
        "kubecove",
        24 * 1024 * 1024,
    ));
    for index in 1..=100 {
        let identity = if index % 5 == 0 {
            format!("WebKitWebProcess --renderer-index={index}")
        } else {
            format!("worker-agent --task={index} --label=worker")
        };
        processes.push(process(
            ROOT_PID + index,
            Some(ROOT_PID + index - 1),
            ROOT_START + u64::from(index),
            &identity,
            4 * 1024 * 1024 + u64::from(index) * 64 * 1024,
        ));
    }
    for index in 101..=220 {
        let identity = if index % 3 == 0 {
            format!("WebKitWebProcess --wide-worker={index}")
        } else {
            format!("worker-agent --wide-worker={index}")
        };
        processes.push(process(
            ROOT_PID + index,
            Some(ROOT_PID),
            ROOT_START + u64::from(index),
            &identity,
            6 * 1024 * 1024 + u64::from(index) * 32 * 1024,
        ));
    }
    processes.extend([
        process(
            ROOT_PID + 221,
            None,
            ROOT_START + 2,
            "com.apple.WebKit.WebContent webcontent.xpc",
            12 * 1024 * 1024,
        ),
        process(
            ROOT_PID + 222,
            None,
            ROOT_START + 2,
            "com.apple.WebKit.GPU webkitgpuprocess",
            10 * 1024 * 1024,
        ),
        process(
            ROOT_PID + 223,
            None,
            ROOT_START + 2,
            "com.apple.WebKit.Networking webkitnetworkprocess",
            8 * 1024 * 1024,
        ),
        process(
            ROOT_PID + 224,
            None,
            ROOT_START + 2,
            "com.apple.WebKit.Storage webkitstorageprocess",
            7 * 1024 * 1024,
        ),
        process(
            ROOT_PID + 225,
            None,
            ROOT_START + 31,
            "com.apple.WebKit.WebContent webcontent.xpc",
            3 * 1024 * 1024,
        ),
        process(
            ROOT_PID + 226,
            Some(ROOT_PID + 227),
            ROOT_START + 10,
            "cycle-left",
            2 * 1024 * 1024,
        ),
        process(
            ROOT_PID + 227,
            Some(ROOT_PID + 226),
            ROOT_START + 10,
            "cycle-right",
            2 * 1024 * 1024,
        ),
        process(
            ROOT_PID + 228,
            None,
            ROOT_START + 2,
            "msedgewebview2.exe --embedded-browser-webview=1 --webview-exe-name=kubecove.exe",
            14 * 1024 * 1024,
        ),
        process(
            ROOT_PID + 229,
            Some(ROOT_PID + 228),
            ROOT_START + 3,
            "msedgewebview2.exe --type=renderer --webview-exe-name=kubecove.exe",
            9 * 1024 * 1024,
        ),
    ]);
    for index in 230..=249 {
        processes.push(process(
            ROOT_PID + index,
            None,
            ROOT_START + u64::from(index),
            &format!("unrelated-daemon --index={index}"),
            1024 * 1024,
        ));
    }
    assert_eq!(processes.len(), PROCESS_TABLE_ROWS);
    processes.reverse();
    let processes = processes
        .into_iter()
        .map(|process| (process.pid, process))
        .collect();
    UsageProcessInputs {
        processes,
        root_pid: ROOT_PID,
        windows_host_names: vec!["kubecove.exe".to_string()],
        cpu_count: 8,
    }
}

pub fn ambiguous_macos_cohort_inputs() -> UsageProcessInputs {
    let mut inputs = process_inputs();
    for (offset, identity) in [
        "com.apple.webkit.webcontent.xpc",
        "com.apple.webkit.gpu",
        "com.apple.webkit.networking",
    ]
    .into_iter()
    .enumerate()
    {
        let pid = 10_230 + u32::try_from(offset).expect("cohort offset fits PID");
        let process = inputs
            .processes
            .get_mut(&pid)
            .expect("ambiguous cohort fixture process");
        process.start_time = 1_004;
        process.process_name = identity.to_string();
        process.command.clear();
    }
    inputs
}

pub fn attribute_processes(
    inputs: UsageProcessInputs,
    platform: UsageProcessPlatform,
) -> AppUsageMetrics {
    let platform = match platform {
        UsageProcessPlatform::Native => BenchUsagePlatform::Native,
        UsageProcessPlatform::MacOs => BenchUsagePlatform::MacOs,
        UsageProcessPlatform::Windows => BenchUsagePlatform::Windows,
    };
    benchmark_usage_process_attribution(
        inputs.processes,
        inputs.root_pid,
        inputs.windows_host_names,
        platform,
        inputs.cpu_count,
    )
}

pub fn encode<T: serde::Serialize>(value: &T) -> String {
    super::encode_complete_response(value)
}

fn process(
    pid: u32,
    parent_pid: Option<u32>,
    start_time: u64,
    identity_text: &str,
    memory_bytes: u64,
) -> BenchUsageProcess {
    let mut identity_parts = identity_text.split_whitespace();
    let process_name = identity_parts.next().unwrap_or_default().to_string();
    BenchUsageProcess {
        pid,
        parent_pid,
        start_time,
        process_name,
        command: identity_parts.map(str::to_string).collect(),
        raw_cpu_percent: 0.75,
        memory_bytes,
    }
}
