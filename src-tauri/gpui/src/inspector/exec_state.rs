//! `ExecSessionTerminal.svelte`: each terminal owns its stream, identity and task.
use super::{Inspector, detail::Detail, exec_leave::LeaveDialog, request::InFlight};
use gpui_kit::{Context, FocusHandle, SharedString, Task, Window};
use kubecove_lib::models::*;
use std::num::NonZeroU16;

// `ExecSessionTerminal.svelte` DEFAULT_COLS/DEFAULT_ROWS. `execTerminal.ts` does
// not override xterm's documented scrollback default, 1000 rows.
pub(super) const COLS: u16 = 100;
pub(super) const ROWS: u16 = 32;
const SCROLLBACK: usize = 1000;

pub(super) struct ExecTab {
    pub key: usize,
    pub container: String,
    pub command: Vec<String>,
    pub phase: ExecPhase,
    /// Starts the session, then reads its messages until the channel closes. Dropping it
    /// drops the stream, which ends the session. `None` before the start and after Stop.
    pub stream: Option<InFlight>,
    pub error: Option<ExecError>,
    pub parser: super::exec_reflow::Parser,
    pub selection: Option<super::exec_selection::Selection>,
    pub lines: Vec<SharedString>,
    pub rendered: Vec<super::exec_buffer::TerminalRow>,
    pub accessible: SharedString,
    pub cursor_visible: bool,
}

/// The terminal's failure, shown through `FriendlyError` as the web shows `error`.
/// Backend errors keep their kind; the web passes the text of stream errors alone.
#[derive(Clone)]
pub(super) struct ExecError {
    pub kind: Option<AppErrorKind>,
    pub message: String,
}
impl From<AppError> for ExecError {
    fn from(error: AppError) -> Self {
        Self {
            kind: Some(error.kind),
            message: error.message,
        }
    }
}
impl From<String> for ExecError {
    fn from(message: String) -> Self {
        Self {
            kind: None,
            message,
        }
    }
}
impl From<&str> for ExecError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

/// The session behind one terminal. `ExecSessionTerminal.svelte` keeps the same
/// states as `sessionId` plus `PodExecSessionStatus | "stopped"`.
pub(super) enum ExecPhase {
    /// Waiting for the first layout to size the terminal.
    Pending(Box<PodExecSessionRequest>),
    /// The start request runs; the backend has not named the session yet.
    Starting,
    Live(LiveExec),
    Ended(ExecEnd),
}

pub(super) struct LiveExec {
    pub id: String,
    pub status: PodExecSessionStatus,
    pub input: Option<ExecInput>,
    pub resize: Option<super::exec_resize::ExecResize>,
}

/// Guarded stdin: keystrokes queue here and one task writes them in order.
pub(super) struct ExecInput {
    sender: tokio::sync::mpsc::UnboundedSender<String>,
    _task: InFlight,
}

#[derive(Clone, Copy)]
pub(super) enum ExecEnd {
    Exited,
    Stopped,
    Error,
}

pub(super) struct ExecState {
    pub custom: Option<super::exec_custom::ExecCustomForm>,
    pub tabs: Vec<ExecTab>,
    pub active: Option<usize>,
    pub next: usize,
    pub focus: FocusHandle,
    pub menu: bool,
    pub composition: Option<String>,
    pub leave: LeaveDialog,
    pub bounds: Option<gpui_kit::Bounds<gpui_kit::Pixels>>,
    pub drag: Option<super::exec_drag::Drag>,
    pub blink_task: Option<Task<()>>,
    /// Selects the cursor paint; xterm draws `cursorInactiveStyle` without focus.
    pub focused: bool,
    _focus: [gpui_kit::Subscription; 2],
}
impl ExecState {
    pub fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let focus = cx.focus_handle();
        let focus_changed = |focused| {
            move |this: &mut Inspector, _: &mut Window, cx: &mut Context<Inspector>| {
                this.exec.focused = focused;
                cx.notify();
            }
        };
        Self {
            custom: None,
            tabs: vec![],
            active: None,
            next: 0,
            _focus: [
                cx.on_focus_in(&focus, window, focus_changed(true)),
                cx.on_blur(&focus, window, focus_changed(false)),
            ],
            focus,
            menu: false,
            composition: None,
            leave: LeaveDialog::Closed,
            bounds: None,
            drag: None,
            blink_task: None,
            focused: false,
        }
    }
    pub fn active(&self) -> Option<&ExecTab> {
        self.tabs.iter().find(|t| Some(t.key) == self.active)
    }
    pub fn clear(&mut self) {
        self.drag = None;
        self.blink_task = None;
        self.custom = None;
        self.tabs.clear();
        self.active = None;
        self.leave = LeaveDialog::Closed;
    }
}
impl ExecTab {
    pub(super) fn session_id(&self) -> Option<&str> {
        match &self.phase {
            ExecPhase::Live(live) => Some(&live.id),
            _ => None,
        }
    }
    /// Whether leaving the view must stop this terminal first. The web asks only for
    /// tabs with a session id; a start in flight counts here too, so leaving never
    /// orphans the session it is about to name.
    pub(super) fn blocks_leave(&self) -> bool {
        !matches!(self.phase, ExecPhase::Ended(_))
    }
    /// The status text and dot of `ExecTab.svelte`.
    pub(super) fn status_label(&self) -> &'static str {
        match &self.phase {
            ExecPhase::Pending(_) | ExecPhase::Starting => "starting",
            ExecPhase::Live(live) => crate::live_sessions::exec_status(live.status),
            ExecPhase::Ended(ExecEnd::Exited) => "exited",
            ExecPhase::Ended(ExecEnd::Stopped) => "stopped",
            ExecPhase::Ended(ExecEnd::Error) => "error",
        }
    }
    /// `stopSession`'s `ended("stopped", "stopping session")`: the tab shows stopped at once
    /// and stops reading input, and the caller stops the returned session. The stream stays
    /// open until that stop returns, because dropping it ends the process even when the
    /// backend stop would fail.
    pub(super) fn stopping(&mut self) -> Option<String> {
        match std::mem::replace(&mut self.phase, ExecPhase::Ended(ExecEnd::Stopped)) {
            ExecPhase::Live(live) => {
                self.note("stopping session");
                Some(live.id)
            }
            phase => {
                self.phase = phase;
                None
            }
        }
    }
    /// `stopBackendSession`'s result for the session `stopping` returned. Success also
    /// clears an earlier stop failure. A failure tracks the session again, as the web
    /// restores `sessionId`, so the next Stop or leave tries again. Returns whether it stopped.
    pub(super) fn stop_returned(&mut self, id: String, result: Result<bool, AppError>) -> bool {
        let error: ExecError = match result {
            Ok(true) => {
                self.stream = None;
                self.error = None;
                self.output("\x1b[2m[session stopped]\x1b[0m\r\n");
                return true;
            }
            Ok(false) => "Stop failed; the session may still be running".into(),
            Err(error) => error.into(),
        };
        self.phase = ExecPhase::Live(LiveExec {
            id,
            status: PodExecSessionStatus::Error,
            input: None,
            resize: None,
        });
        self.error = Some(error);
        self.note("stop failed; the session may still be running");
        false
    }
    /// `writeln` of a dim bracketed note on its own line.
    fn note(&mut self, note: &str) {
        self.output(&format!("\r\n\x1b[2m[{note}]\x1b[0m\r\n"));
    }
    pub(super) fn refresh_view(&mut self) {
        if self
            .selection
            .is_some_and(|s| s.ordered().0.0 < self.parser.callbacks().discarded)
        {
            self.selection = None;
        }
        self.rendered = super::exec_buffer::snapshot(
            self.parser.screen(),
            self.selection,
            self.viewport_start(),
        );
        self.lines = self.rendered.iter().map(|row| row.text.clone()).collect();
        self.accessible = self
            .lines
            .iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>()
            .join("\n")
            .into();
    }
    fn output(&mut self, data: &str) {
        let alternate = self.parser.screen().alternate_screen();
        // `execTerminal.ts` enables xterm's convertEol.
        for part in data.split_inclusive('\n') {
            if let Some(line) = part.strip_suffix('\n') {
                self.parser.process(line.as_bytes());
                self.parser.process(b"\r\n");
            } else {
                self.parser.process(part.as_bytes());
            }
        }
        if self.parser.screen().alternate_screen() != alternate {
            self.selection = None;
        }
        // Cache viewport rows only on output, never copy/reflow scrollback in render.
        self.refresh_view();
    }
    fn message(&mut self, event: PodExecSessionMessage) {
        let id = match &event {
            PodExecSessionMessage::Started { session_id, .. }
            | PodExecSessionMessage::Status { session_id, .. }
            | PodExecSessionMessage::Output { session_id, .. }
            | PodExecSessionMessage::Error { session_id, .. }
            | PodExecSessionMessage::Exited { session_id, .. }
            | PodExecSessionMessage::Stopped { session_id } => session_id,
        };
        if self.session_id() != Some(id.as_str()) {
            return;
        }
        if let PodExecSessionMessage::Output { data, .. } = &event {
            self.output(data);
            return;
        }
        let ExecPhase::Live(live) = &mut self.phase else {
            return;
        };
        match event {
            PodExecSessionMessage::Started { summary, .. } => live.status = summary.status,
            PodExecSessionMessage::Status { status, .. } => live.status = status,
            PodExecSessionMessage::Error { message, .. } => {
                live.status = PodExecSessionStatus::Error;
                self.error = Some(message.into());
            }
            PodExecSessionMessage::Exited { .. } => self.phase = ExecPhase::Ended(ExecEnd::Exited),
            PodExecSessionMessage::Stopped { .. } => {
                self.phase = ExecPhase::Ended(ExecEnd::Stopped)
            }
            PodExecSessionMessage::Output { .. } => {}
        }
    }
    /// The stream closed without a final message.
    fn stream_closed(&mut self) {
        self.stream = None;
        if let ExecPhase::Live(live) = &self.phase {
            self.phase = ExecPhase::Ended(match live.status {
                PodExecSessionStatus::Error => ExecEnd::Error,
                PodExecSessionStatus::Exited => ExecEnd::Exited,
                _ => ExecEnd::Stopped,
            });
        }
    }
}

impl Inspector {
    pub(super) fn start_exec(
        &mut self,
        container: String,
        command: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Detail::Ready(detail) = &self.detail else {
            return;
        };
        let resource = &detail.data.summary;
        if resource.kind != "Pod" || resource.namespace.is_none() || command.is_empty() {
            return;
        }
        let target = format!(
            "{}/{}/Pod/{}/container/{}",
            resource.cluster,
            resource.namespace.as_deref().unwrap_or_default(),
            resource.name,
            if container.is_empty() {
                "<default>"
            } else {
                &container
            }
        );
        let request = PodExecSessionRequest {
            cluster_context: resource.cluster.clone(),
            kubeconfig_env_var: self.session_env_var(),
            namespace: resource.namespace.clone().unwrap_or_default(),
            pod_name: resource.name.clone(),
            container: (!container.is_empty()).then(|| container.clone()),
            command: command.clone(),
            stdin: true,
            tty: true,
            terminal_size: PodExecTerminalSize {
                cols: COLS,
                rows: ROWS,
            },
            confirmation: PodExecConfirmation {
                acknowledged: true,
                target,
                command: crate::live_sessions::exec_command(&command),
            },
        };
        let key = self.exec.next;
        // The detail has a 1px divider, p-4 content, and a 1px terminal border.
        // Fit before printing the start notice so a subsequent shrink cannot truncate it.
        let width = super::resources::detail_width(self, window) - 1. - 2. * 16. - 2.;
        let cols = super::exec_resize::terminal_columns(width, window).unwrap_or(COLS);
        self.exec.next += 1;
        self.exec.active = Some(key);
        self.exec.menu = false;
        let mut tab = ExecTab {
            key,
            container,
            command,
            phase: ExecPhase::Ended(ExecEnd::Stopped),
            stream: None,
            error: None,
            parser: super::exec_reflow::Parser::new_with_callbacks(
                NonZeroU16::new(ROWS).unwrap(),
                NonZeroU16::new(cols).unwrap(),
                SCROLLBACK,
                Default::default(),
            ),
            selection: None,
            lines: vec![],
            rendered: vec![],
            accessible: "".into(),
            cursor_visible: true,
        };
        tab.output(&format!(
            "\x1b[2mStarting {} on {}\x1b[0m\r\n\r\n",
            tab.command.join(" "),
            request.confirmation.target
        ));
        tab.phase = ExecPhase::Pending(Box::new(request));
        self.exec.tabs.push(tab);
        if self.exec.blink_task.is_none() {
            self.exec.blink_task = Some(cx.spawn(async move |this, cx| {
                loop {
                    // xterm DomRenderer's 1s step-end animation changes at 50%.
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    if !this
                        .update(cx, |this, cx| {
                            if this.exec.tabs.is_empty() {
                                return false;
                            }
                            if let Some(tab) = this
                                .exec
                                .tabs
                                .iter_mut()
                                .find(|t| Some(t.key) == this.exec.active)
                            {
                                tab.cursor_visible = !tab.cursor_visible;
                                cx.notify();
                            }
                            true
                        })
                        .unwrap_or(false)
                    {
                        break;
                    }
                }
            }));
        }
        window.focus(&self.exec.focus, cx);
        cx.notify();
    }

    pub(super) fn begin_exec_stream(&mut self, key: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.exec.tabs.iter_mut().find(|t| t.key == key) else {
            return;
        };
        let mut request = match std::mem::replace(&mut tab.phase, ExecPhase::Starting) {
            ExecPhase::Pending(request) => request,
            phase => {
                tab.phase = phase;
                return;
            }
        };
        let (rows, cols) = tab.parser.screen().size();
        request.terminal_size = PodExecTerminalSize {
            cols: cols.get(),
            rows: rows.get(),
        };
        let read = self.backend.exec(*request);
        let stream = cx.spawn(async move |this, cx| match read.await {
            Ok(mut stream) => {
                let id = stream.summary.id.clone();
                let alive = this
                    .update(cx, |this, cx| {
                        let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) else {
                            return false;
                        };
                        tab.phase = ExecPhase::Live(LiveExec {
                            id,
                            status: stream.summary.status,
                            input: None,
                            resize: None,
                        });
                        this.open_exec_input(key, cx);
                        this.send_exec_resize(key, cx);
                        this.load_sessions(cx);
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    return;
                }
                while let Some(event) = stream.messages.recv().await {
                    let alive = this
                        .update(cx, |this, cx| {
                            let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) else {
                                return false;
                            };
                            tab.message(event);
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if !alive {
                        break;
                    }
                    // A buffered output burst must yield so view closure can cancel it.
                    tokio::task::yield_now().await;
                }
                let _ = this.update(cx, |this, cx| {
                    if let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) {
                        tab.stream_closed();
                    }
                    this.load_sessions(cx);
                    cx.notify();
                });
            }
            Err(error) => {
                let _ = this.update(cx, |this, cx| {
                    if let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) {
                        tab.stream = None;
                        tab.phase = ExecPhase::Ended(ExecEnd::Error);
                        tab.error = Some(error.into());
                    }
                    cx.notify();
                });
            }
        });
        tab.stream = Some(stream.into());
        cx.notify();
    }

    pub(super) fn exec_input(&mut self, data: String, cx: &mut Context<Self>) {
        let Some(tab) = self
            .exec
            .tabs
            .iter_mut()
            .find(|t| Some(t.key) == self.exec.active)
        else {
            return;
        };
        if let ExecPhase::Live(LiveExec {
            input: Some(input), ..
        }) = &tab.phase
        {
            let _ = input.sender.send(data);
        }
        tab.parser.screen_mut().set_scrollback(0);
        tab.selection = None;
        tab.refresh_view();
        cx.notify();
    }
    fn open_exec_input(&mut self, key: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.exec.tabs.iter_mut().find(|t| t.key == key) else {
            return;
        };
        let ExecPhase::Live(live) = &mut tab.phase else {
            return;
        };
        let id = live.id.clone();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let backend = self.backend.clone();
        let task = cx.spawn(async move |this, cx| {
            while let Some(data) = receiver.recv().await {
                if let Err(error) = backend.exec_input(id.clone(), data).await {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) {
                            tab.error = Some(error.into());
                        }
                        cx.notify();
                    });
                    break;
                }
            }
        });
        live.input = Some(ExecInput {
            sender,
            _task: task.into(),
        });
    }
    pub(super) fn stop_exec_tab(&mut self, key: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.exec.tabs.iter_mut().find(|t| t.key == key) else {
            return;
        };
        let Some(id) = tab.stopping() else {
            return;
        };
        let read = self.backend.stop_exec(id.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) {
                    tab.stop_returned(id, result);
                }
                this.load_sessions(cx);
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn close_exec(&mut self, key: usize, cx: &mut Context<Self>) {
        let Some(index) = self.exec.tabs.iter().position(|t| t.key == key) else {
            return;
        };
        self.exec.tabs.remove(index);
        if self.exec.tabs.is_empty() {
            self.exec.blink_task = None;
        }
        if self.exec.active == Some(key) {
            self.exec.active = self
                .exec
                .tabs
                .get(index)
                .or_else(|| index.checked_sub(1).and_then(|i| self.exec.tabs.get(i)))
                .map(|t| t.key);
        }
        self.load_sessions(cx);
        cx.notify();
    }
}
