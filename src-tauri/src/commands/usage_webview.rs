use sysinfo::Process;

pub(super) const ORPHAN_WEBVIEW_START_WINDOW_SECONDS: u64 = 30;

const WEBVIEW_PROCESS_MARKERS: &[&str] = &[
    "webview",
    "msedgewebview",
    "webkitwebprocess",
    "webkitnetworkprocess",
    "webkitgpuprocess",
    "webkitstorageprocess",
    "webkit.framework",
    "com.apple.webkit.",
];

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn process_name_lossy(process: &Process) -> String {
    process.name().to_string_lossy().into_owned()
}

fn process_cmd_text(process: &Process) -> String {
    process
        .cmd()
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn process_identity_text(process: &Process) -> String {
    format!(
        "{} {}",
        process_name_lossy(process),
        process_cmd_text(process)
    )
}

pub(super) fn is_webview_process(process: &Process) -> bool {
    is_webview_process_text(&process_identity_text(process))
}

pub(super) fn is_webview_process_text(process_text: &str) -> bool {
    let name = process_text.to_ascii_lowercase();
    contains_any(&name, WEBVIEW_PROCESS_MARKERS)
}

pub(super) fn webview_process_role(process: &Process) -> Option<String> {
    let identity = process_identity_text(process);
    if !is_webview_process_text(&identity) {
        return None;
    }
    webview_process_role_from_cmd(&identity).or_else(|| Some("WebView process".to_string()))
}

pub(super) fn webview_process_role_from_cmd(command_line: &str) -> Option<String> {
    let text = command_line.to_ascii_lowercase();

    if text.contains("--type=renderer") {
        return Some("Renderer".to_string());
    }
    if text.contains("--type=gpu-process") {
        return Some("GPU process".to_string());
    }
    if text.contains("--type=utility") && text.contains("network.mojom.networkservice") {
        return Some("Network service".to_string());
    }
    if text.contains("--type=utility") && text.contains("storage.mojom.storageservice") {
        return Some("Storage service".to_string());
    }
    if text.contains("--type=utility") && text.contains("audio.mojom.audioservice") {
        return Some("Audio service".to_string());
    }
    if text.contains("--type=utility") {
        return Some("Utility process".to_string());
    }
    if text.contains("--type=crashpad-handler") || text.contains("crashpad") {
        return Some("Crash handler".to_string());
    }
    if contains_any(
        &text,
        &[
            "webkitwebprocess",
            "com.apple.webkit.webcontent",
            "webcontent.xpc",
        ],
    ) {
        return Some("Renderer".to_string());
    }
    if contains_any(
        &text,
        &["webkitgpuprocess", "com.apple.webkit.gpu", "gpu.xpc"],
    ) {
        return Some("GPU process".to_string());
    }
    if contains_any(
        &text,
        &[
            "webkitnetworkprocess",
            "com.apple.webkit.networking",
            "networking.xpc",
        ],
    ) {
        return Some("Network service".to_string());
    }
    if text.contains("webkitstorageprocess") || text.contains("webkitstorageservice") {
        return Some("Storage service".to_string());
    }
    if text.contains("msedgewebview2") || text.contains("webview") {
        return Some("Browser process".to_string());
    }
    if is_webview_process_text(&text) {
        return Some("WebKit process".to_string());
    }

    None
}

pub(super) fn is_orphan_webview_start_candidate(
    root_start_time: u64,
    process_start_time: u64,
) -> bool {
    process_start_time >= root_start_time
        && process_start_time.saturating_sub(root_start_time) <= ORPHAN_WEBVIEW_START_WINDOW_SECONDS
}

pub(super) fn selected_orphan_webview_start_time(
    root_start_time: u64,
    process_start_times: impl IntoIterator<Item = u64>,
) -> Option<u64> {
    process_start_times
        .into_iter()
        .filter(|start_time| is_orphan_webview_start_candidate(root_start_time, *start_time))
        .min()
}

#[cfg(test)]
mod tests {
    use super::{
        is_orphan_webview_start_candidate, is_webview_process_text,
        selected_orphan_webview_start_time, webview_process_role_from_cmd,
    };

    #[test]
    fn identifies_webview_process_names() {
        for process_name in [
            "msedgewebview2.exe",
            "Microsoft.WebView2",
            "WebKitWebProcess",
            "WebKitNetworkProcess",
            "/System/Library/Frameworks/WebKit.framework/Versions/A/XPCServices/com.apple.WebKit.WebContent.xpc/Contents/MacOS/com.apple.WebKit.WebContent",
        ] {
            assert!(is_webview_process_text(process_name));
        }
        assert!(!is_webview_process_text("kubecove.exe"));
    }

    #[test]
    fn classifies_webview_roles_from_command_flags() {
        for (command, role) in [
            ("msedgewebview2.exe --type=renderer", "Renderer"),
            ("msedgewebview2.exe --type=gpu-process", "GPU process"),
            (
                "msedgewebview2.exe --type=utility --utility-sub-type=network.mojom.NetworkService",
                "Network service",
            ),
            ("WebKitWebProcess", "Renderer"),
            ("com.apple.WebKit.GPU", "GPU process"),
            ("com.apple.WebKit.Networking", "Network service"),
        ] {
            assert_eq!(
                webview_process_role_from_cmd(command),
                Some(role.to_string())
            );
        }
    }

    #[test]
    fn selects_nearest_orphan_webview_start_time_after_host_start() {
        assert!(is_orphan_webview_start_candidate(100, 102));
        assert!(!is_orphan_webview_start_candidate(100, 99));
        assert!(!is_orphan_webview_start_candidate(100, 131));
        assert_eq!(
            selected_orphan_webview_start_time(100, [50, 102, 105, 132]),
            Some(102),
        );
        assert_eq!(selected_orphan_webview_start_time(100, [50, 99, 132]), None);
    }
}
