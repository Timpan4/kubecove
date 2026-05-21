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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OrphanWebViewProcessCandidate {
    pub start_time: u64,
    pub role: WebViewProcessRole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WebViewProcessRole {
    Renderer,
    Gpu,
    Network,
    Storage,
    Audio,
    Utility,
    Crash,
    Browser,
    WebKit,
}

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
    webview_process_role_from_cmd(&identity)
}

pub(super) fn orphan_webview_process_candidate(
    process: &Process,
) -> Option<OrphanWebViewProcessCandidate> {
    let identity = process_identity_text(process);
    let role = webview_process_role_from_text(&identity)?;
    Some(OrphanWebViewProcessCandidate {
        start_time: process.start_time(),
        role,
    })
}

pub(super) fn webview_process_role_from_cmd(command_line: &str) -> Option<String> {
    webview_process_role_from_text(command_line).map(|role| role.label().to_string())
}

fn webview_process_role_from_text(command_line: &str) -> Option<WebViewProcessRole> {
    let text = command_line.to_ascii_lowercase();
    if text.contains("--type=renderer") {
        return Some(WebViewProcessRole::Renderer);
    }
    if text.contains("--type=gpu-process") {
        return Some(WebViewProcessRole::Gpu);
    }
    if text.contains("--type=utility") && text.contains("network.mojom.networkservice") {
        return Some(WebViewProcessRole::Network);
    }
    if text.contains("--type=utility") && text.contains("storage.mojom.storageservice") {
        return Some(WebViewProcessRole::Storage);
    }
    if text.contains("--type=utility") && text.contains("audio.mojom.audioservice") {
        return Some(WebViewProcessRole::Audio);
    }
    if text.contains("--type=utility") {
        return Some(WebViewProcessRole::Utility);
    }
    if text.contains("--type=crashpad-handler") || text.contains("crashpad") {
        return Some(WebViewProcessRole::Crash);
    }
    if contains_any(
        &text,
        &[
            "webkitwebprocess",
            "com.apple.webkit.webcontent",
            "webcontent.xpc",
        ],
    ) {
        return Some(WebViewProcessRole::Renderer);
    }
    if contains_any(
        &text,
        &["webkitgpuprocess", "com.apple.webkit.gpu", "gpu.xpc"],
    ) {
        return Some(WebViewProcessRole::Gpu);
    }
    if contains_any(
        &text,
        &[
            "webkitnetworkprocess",
            "com.apple.webkit.networking",
            "networking.xpc",
        ],
    ) {
        return Some(WebViewProcessRole::Network);
    }
    if text.contains("webkitstorageprocess") || text.contains("webkitstorageservice") {
        return Some(WebViewProcessRole::Storage);
    }
    if text.contains("msedgewebview2") || text.contains("webview") {
        return Some(WebViewProcessRole::Browser);
    }
    if is_webview_process_text(&text) {
        return Some(WebViewProcessRole::WebKit);
    }

    None
}

impl WebViewProcessRole {
    fn label(self) -> &'static str {
        match self {
            Self::Renderer => "Renderer",
            Self::Gpu => "GPU process",
            Self::Network => "Network service",
            Self::Storage => "Storage service",
            Self::Audio => "Audio service",
            Self::Utility => "Utility process",
            Self::Crash => "Crash handler",
            Self::Browser => "Browser process",
            Self::WebKit => "WebKit process",
        }
    }
}

pub(super) fn is_orphan_webview_start_candidate(
    root_start_time: u64,
    process_start_time: u64,
) -> bool {
    process_start_time >= root_start_time
        && process_start_time.saturating_sub(root_start_time) <= ORPHAN_WEBVIEW_START_WINDOW_SECONDS
}

pub(super) fn selected_orphan_webview_cohort_start_time(
    root_start_time: u64,
    candidates: impl IntoIterator<Item = OrphanWebViewProcessCandidate>,
) -> Option<u64> {
    let mut candidate_start_times = candidates
        .into_iter()
        .filter(|candidate| {
            is_orphan_webview_start_candidate(root_start_time, candidate.start_time)
        })
        .fold(
            Vec::<(u64, WebViewCohort)>::new(),
            |mut cohorts, candidate| {
                if let Some((_, cohort)) = cohorts
                    .iter_mut()
                    .find(|(start_time, _)| *start_time == candidate.start_time)
                {
                    cohort.add(candidate.role);
                } else {
                    let mut cohort = WebViewCohort::default();
                    cohort.add(candidate.role);
                    cohorts.push((candidate.start_time, cohort));
                }
                cohorts
            },
        );

    candidate_start_times.retain(|(_, cohort)| cohort.is_unambiguous());
    if candidate_start_times.len() == 1 {
        Some(candidate_start_times[0].0)
    } else {
        None
    }
}

#[derive(Default)]
struct WebViewCohort {
    renderer_count: usize,
    gpu_count: usize,
    network_count: usize,
    extra_count: usize,
}

impl WebViewCohort {
    fn add(&mut self, role: WebViewProcessRole) {
        match role {
            WebViewProcessRole::Renderer => self.renderer_count += 1,
            WebViewProcessRole::Gpu => self.gpu_count += 1,
            WebViewProcessRole::Network => self.network_count += 1,
            _ => self.extra_count += 1,
        }
    }

    fn is_unambiguous(&self) -> bool {
        self.renderer_count == 1
            && self.gpu_count <= 1
            && self.network_count <= 1
            && self.extra_count == 0
            && self.gpu_count + self.network_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_orphan_webview_start_candidate, is_webview_process_text,
        selected_orphan_webview_cohort_start_time, webview_process_role_from_cmd,
        OrphanWebViewProcessCandidate, WebViewProcessRole,
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
    fn selects_single_unambiguous_orphan_webview_cohort_after_host_start() {
        assert!(is_orphan_webview_start_candidate(100, 102));
        assert!(!is_orphan_webview_start_candidate(100, 99));
        assert!(!is_orphan_webview_start_candidate(100, 131));
        assert_eq!(
            selected_orphan_webview_cohort_start_time(
                100,
                [
                    candidate(50, WebViewProcessRole::Renderer),
                    candidate(102, WebViewProcessRole::Renderer),
                    candidate(102, WebViewProcessRole::Gpu),
                    candidate(102, WebViewProcessRole::Network),
                    candidate(132, WebViewProcessRole::Renderer),
                ],
            ),
            Some(102),
        );
    }

    #[test]
    fn ignores_ambiguous_orphan_webview_cohorts() {
        assert_eq!(
            selected_orphan_webview_cohort_start_time(
                100,
                [
                    candidate(102, WebViewProcessRole::Renderer),
                    candidate(102, WebViewProcessRole::Renderer),
                    candidate(102, WebViewProcessRole::Gpu),
                ],
            ),
            None,
        );
        assert_eq!(
            selected_orphan_webview_cohort_start_time(
                100,
                [
                    candidate(102, WebViewProcessRole::Renderer),
                    candidate(102, WebViewProcessRole::Gpu),
                    candidate(105, WebViewProcessRole::Renderer),
                    candidate(105, WebViewProcessRole::Network),
                ],
            ),
            None,
        );
        assert_eq!(
            selected_orphan_webview_cohort_start_time(
                100,
                [
                    candidate(99, WebViewProcessRole::Renderer),
                    candidate(132, WebViewProcessRole::Gpu),
                ],
            ),
            None,
        );
    }

    fn candidate(start_time: u64, role: WebViewProcessRole) -> OrphanWebViewProcessCandidate {
        OrphanWebViewProcessCandidate { start_time, role }
    }
}
