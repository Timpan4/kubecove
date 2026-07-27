use sysinfo::Process;

pub(super) const ORPHAN_WEBVIEW_START_WINDOW_SECONDS: u64 = 30;

const DESCENDANT_WEBVIEW_PROCESS_MARKERS: &[&str] = &[
    "webview",
    "msedgewebview",
    "webkitwebprocess",
    "webkitnetworkprocess",
    "webkitgpuprocess",
    "webkitstorageprocess",
    "webkit.framework",
    "com.apple.webkit.",
];

// macOS orphan WebKit detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
const MACOS_ORPHAN_WEBKIT_PROCESS_MARKERS: &[&str] = &[
    "com.apple.webkit.webcontent",
    "com.apple.webkit.gpu",
    "com.apple.webkit.networking",
    "com.apple.webkit.storage",
];

// Windows WebView2 detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
const WINDOWS_WEBVIEW_PROCESS_MARKER: &str = "msedgewebview2";
// Windows WebView2 detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
const WINDOWS_WEBVIEW_HOST_EXE_ARG: &str = "--webview-exe-name=";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// macOS orphan WebKit detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
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

pub(super) fn is_webview_descendant_process(process: &Process) -> bool {
    is_webview_descendant_process_text(&process_identity_text(process))
}

pub(super) fn is_webview_descendant_process_text(process_text: &str) -> bool {
    let name = process_text.to_ascii_lowercase();
    contains_any(&name, DESCENDANT_WEBVIEW_PROCESS_MARKERS)
}

// macOS orphan WebKit detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) fn is_macos_orphan_webkit_process(process: &Process) -> bool {
    is_macos_orphan_webkit_process_text(&process_identity_text(process))
}

// macOS orphan WebKit detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) fn is_macos_orphan_webkit_process_text(process_text: &str) -> bool {
    let name = process_text.to_ascii_lowercase();
    contains_any(&name, MACOS_ORPHAN_WEBKIT_PROCESS_MARKERS)
}

// Windows WebView2 detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) fn windows_webview_host_exe_names(process: &Process) -> Vec<String> {
    let mut names = Vec::new();
    add_windows_host_exe_name(&mut names, &process_name_lossy(process));
    if let Some(exe_name) = process
        .exe()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
    {
        add_windows_host_exe_name(&mut names, &exe_name);
    }
    if let Some(cmd_exe_name) = process
        .cmd()
        .first()
        .and_then(|part| std::path::Path::new(part).file_name())
        .map(|name| name.to_string_lossy().into_owned())
    {
        add_windows_host_exe_name(&mut names, &cmd_exe_name);
    }
    names
}

// Windows WebView2 detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
fn add_windows_host_exe_name(names: &mut Vec<String>, name: &str) {
    let name = name.trim_matches('"').to_ascii_lowercase();
    if name.is_empty() {
        return;
    }
    let candidates = if std::path::Path::new(&name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        vec![name]
    } else {
        vec![name.clone(), format!("{name}.exe")]
    };
    for candidate in candidates {
        if !names.contains(&candidate) {
            names.push(candidate);
        }
    }
}

// Windows WebView2 detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) fn is_windows_app_webview_process(process: &Process, host_exe_names: &[String]) -> bool {
    is_windows_app_webview_process_text(host_exe_names, &process_identity_text(process))
}

// Windows WebView2 detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) fn is_windows_app_webview_process_text(
    host_exe_names: &[String],
    process_text: &str,
) -> bool {
    let text = process_text.to_ascii_lowercase();
    text.contains(WINDOWS_WEBVIEW_PROCESS_MARKER)
        && host_exe_names
            .iter()
            .any(|name| text.contains(&format!("{WINDOWS_WEBVIEW_HOST_EXE_ARG}{name}")))
}

// Windows WebView2 detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) fn is_windows_app_webview_browser_process(
    process: &Process,
    host_exe_names: &[String],
) -> bool {
    is_windows_app_webview_browser_process_text(host_exe_names, &process_identity_text(process))
}

// Windows WebView2 detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) fn is_windows_app_webview_browser_process_text(
    host_exe_names: &[String],
    process_text: &str,
) -> bool {
    let text = process_text.to_ascii_lowercase();
    text.contains("--embedded-browser-webview=1")
        && !text.contains("--type=")
        && is_windows_app_webview_process_text(host_exe_names, &text)
}

pub(super) fn webview_process_role(process: &Process) -> Option<String> {
    let identity = process_identity_text(process);
    if !is_webview_descendant_process_text(&identity) {
        return None;
    }
    webview_process_role_from_cmd(&identity)
}

// macOS orphan WebKit detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
pub(super) fn macos_orphan_webview_process_candidate(
    process: &Process,
) -> Option<OrphanWebViewProcessCandidate> {
    let identity = process_identity_text(process);
    if !is_macos_orphan_webkit_process_text(&identity) {
        return None;
    }
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
    if text.contains("webkitstorageprocess")
        || text.contains("webkitstorageservice")
        || text.contains("com.apple.webkit.storage")
    {
        return Some(WebViewProcessRole::Storage);
    }
    if text.contains("msedgewebview2") || text.contains("webview") {
        return Some(WebViewProcessRole::Browser);
    }
    if is_webview_descendant_process_text(&text) {
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

// macOS orphan WebKit detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
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
// macOS orphan WebKit detection is compiled on every platform so shared tests can cover it.
#[allow(dead_code)]
struct WebViewCohort {
    renderer_count: usize,
    gpu_count: usize,
    network_count: usize,
    storage_count: usize,
    auxiliary_count: usize,
    extra_count: usize,
}

impl WebViewCohort {
    // macOS orphan WebKit detection is compiled on every platform so shared tests can cover it.
    #[allow(dead_code)]
    fn add(&mut self, role: WebViewProcessRole) {
        match role {
            WebViewProcessRole::Renderer => self.renderer_count += 1,
            WebViewProcessRole::Gpu => self.gpu_count += 1,
            WebViewProcessRole::Network => self.network_count += 1,
            WebViewProcessRole::Storage => self.storage_count += 1,
            WebViewProcessRole::Audio | WebViewProcessRole::Utility | WebViewProcessRole::Crash => {
                self.auxiliary_count += 1;
            }
            _ => self.extra_count += 1,
        }
    }

    // macOS orphan WebKit detection is compiled on every platform so shared tests can cover it.
    #[allow(dead_code)]
    fn is_unambiguous(&self) -> bool {
        self.renderer_count == 1
            && self.gpu_count <= 1
            && self.network_count <= 1
            && self.storage_count <= 1
            && self.auxiliary_count <= 1
            && self.extra_count == 0
            && self.gpu_count + self.network_count + self.storage_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_macos_orphan_webkit_process_text, is_orphan_webview_start_candidate,
        is_webview_descendant_process_text, is_windows_app_webview_browser_process_text,
        is_windows_app_webview_process_text, selected_orphan_webview_cohort_start_time,
        webview_process_role_from_cmd, OrphanWebViewProcessCandidate, WebViewProcessRole,
    };

    #[test]
    fn identifies_descendant_webview_process_names() {
        for process_name in [
            "msedgewebview2.exe",
            "Microsoft.WebView2",
            "WebKitWebProcess",
            "WebKitNetworkProcess",
            "/System/Library/Frameworks/WebKit.framework/Versions/A/XPCServices/com.apple.WebKit.WebContent.xpc/Contents/MacOS/com.apple.WebKit.WebContent",
        ] {
            assert!(is_webview_descendant_process_text(process_name));
        }
        assert!(!is_webview_descendant_process_text("kubecove.exe"));
    }

    #[test]
    fn only_exact_macos_webkit_helpers_can_be_orphan_candidates() {
        for process_name in [
            "com.apple.WebKit.WebContent",
            "com.apple.WebKit.GPU",
            "com.apple.WebKit.Networking",
            "com.apple.WebKit.Storage",
            "/System/Library/Frameworks/WebKit.framework/Versions/A/XPCServices/com.apple.WebKit.WebContent.xpc/Contents/MacOS/com.apple.WebKit.WebContent",
        ] {
            assert!(is_macos_orphan_webkit_process_text(process_name));
        }

        for process_name in [
            "my-webview-helper",
            "WebKitWebProcess",
            "Microsoft.WebView2",
            "msedgewebview2.exe",
            "webkit.framework",
        ] {
            assert!(!is_macos_orphan_webkit_process_text(process_name));
        }
    }

    #[test]
    fn identifies_windows_webview_processes_for_the_host_exe() {
        let host_names = vec!["kubecove.exe".to_string()];
        assert!(is_windows_app_webview_process_text(
            &host_names,
            r#""C:\Program Files (x86)\Microsoft\EdgeWebView\Application\148.0.3967.96\msedgewebview2.exe" --embedded-browser-webview=1 --webview-exe-name=kubecove.exe --user-data-dir="C:\Users\timpa\AppData\Local\com.timpan.kubecove\EBWebView""#,
        ));
        assert!(is_windows_app_webview_process_text(
            &host_names,
            r#""C:\Program Files (x86)\Microsoft\EdgeWebView\Application\148.0.3967.96\msedgewebview2.exe" --type=renderer --webview-exe-name=kubecove.exe --embedded-browser-webview=1"#,
        ));

        assert!(!is_windows_app_webview_process_text(
            &host_names,
            r#""C:\Program Files (x86)\Microsoft\EdgeWebView\Application\148.0.3967.96\msedgewebview2.exe" --embedded-browser-webview=1 --webview-exe-name=SearchHost.exe"#,
        ));
        assert!(!is_windows_app_webview_process_text(
            &host_names,
            r#""C:\Users\timpa\AppData\Local\KubeCove\kubecove.exe""#,
        ));
    }

    #[test]
    fn accepts_windows_host_name_with_or_without_exe_suffix() {
        let host_names = vec!["kubecove".to_string(), "kubecove.exe".to_string()];
        assert!(is_windows_app_webview_process_text(
            &host_names,
            "msedgewebview2.exe --webview-exe-name=kubecove.exe",
        ));
    }

    #[test]
    fn identifies_windows_webview_browser_processes_for_the_host_exe() {
        let host_names = vec!["kubecove.exe".to_string()];
        assert!(is_windows_app_webview_browser_process_text(
            &host_names,
            r#""C:\Program Files (x86)\Microsoft\EdgeWebView\Application\148.0.3967.96\msedgewebview2.exe" --embedded-browser-webview=1 --webview-exe-name=kubecove.exe"#,
        ));
        assert!(!is_windows_app_webview_browser_process_text(
            &host_names,
            r#""C:\Program Files (x86)\Microsoft\EdgeWebView\Application\148.0.3967.96\msedgewebview2.exe" --type=renderer --embedded-browser-webview=1 --webview-exe-name=kubecove.exe"#,
        ));
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
    fn accepts_storage_helper_in_orphan_webview_cohort() {
        assert_eq!(
            selected_orphan_webview_cohort_start_time(
                100,
                [
                    candidate(102, WebViewProcessRole::Renderer),
                    candidate(102, WebViewProcessRole::Network),
                    candidate(102, WebViewProcessRole::Storage),
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
        assert_eq!(
            selected_orphan_webview_cohort_start_time(
                100,
                [
                    candidate(102, WebViewProcessRole::Renderer),
                    candidate(102, WebViewProcessRole::Network),
                    candidate(102, WebViewProcessRole::Storage),
                    candidate(102, WebViewProcessRole::Storage),
                ],
            ),
            None,
        );
    }

    fn candidate(start_time: u64, role: WebViewProcessRole) -> OrphanWebViewProcessCandidate {
        OrphanWebViewProcessCandidate { start_time, role }
    }
}
