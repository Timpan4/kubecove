//! `ExecSessionTerminal.svelte`: each terminal owns its stream, identity and task.
use super::{Inspector, detail::Detail};
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
    pub id: Option<String>,
    pub container: String,
    pub command: Vec<String>,
    pub status: String,
    pub error: Option<String>,
    pub parser: super::exec_reflow::Parser,
    pub selection: Option<super::exec_selection::Selection>,
    pub lines: Vec<SharedString>,
    pub rendered: Vec<super::exec_buffer::TerminalRow>,
    pub accessible: SharedString,
    pub task: Option<Task<()>>,
    pub stdin: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    pub input_task: Option<Task<()>>,
    pub resize_task: Option<Task<()>>,
    pub resize_sender: Option<tokio::sync::watch::Sender<PodExecTerminalSize>>,
    pub pending_start: Option<PodExecSessionRequest>,
    pub cursor_visible: bool,
}
pub(super) struct ExecState {
    pub custom: Option<super::exec_custom::ExecCustomForm>,
    pub tabs: Vec<ExecTab>,
    pub active: Option<usize>,
    pub next: usize,
    pub focus: FocusHandle,
    pub menu: bool,
    pub composition: Option<String>,
    pub leave: Option<super::exec_leave::LeaveIntent>,
    pub leave_error: Option<String>,
    pub stopping: bool,
    pub bounds: Option<gpui_kit::Bounds<gpui_kit::Pixels>>,
    pub selecting: bool,
    pub blink_task: Option<Task<()>>,
}
impl ExecState {
    pub fn new(cx: &mut Context<Inspector>) -> Self {
        Self {
            custom: None,
            tabs: vec![],
            active: None,
            next: 0,
            focus: cx.focus_handle(),
            menu: false,
            composition: None,
            leave: None,
            leave_error: None,
            stopping: false,
            bounds: None,
            selecting: false,
            blink_task: None,
        }
    }
    pub fn active(&self) -> Option<&ExecTab> {
        self.tabs.iter().find(|t| Some(t.key) == self.active)
    }
    pub fn clear(&mut self) {
        self.blink_task = None;
        self.custom = None;
        self.tabs.clear();
        self.active = None;
    }
}
impl ExecTab {
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
            self.cursor_visible,
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
        if self.id.as_ref() != Some(id) {
            return;
        }
        match event {
            PodExecSessionMessage::Started { summary, .. } => {
                self.status = crate::live_sessions::exec_status(summary.status).into()
            }
            PodExecSessionMessage::Status { status, .. } => {
                self.status = crate::live_sessions::exec_status(status).into()
            }
            PodExecSessionMessage::Output { data, .. } => self.output(&data),
            PodExecSessionMessage::Error { message, .. } => {
                self.status = "error".into();
                self.error = Some(message);
            }
            PodExecSessionMessage::Exited { .. } => {
                self.status = "exited".into();
                self.id = None;
            }
            PodExecSessionMessage::Stopped { .. } => {
                self.status = "stopped".into();
                self.id = None;
            }
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
            id: None,
            container,
            command,
            status: "starting".into(),
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
            task: None,
            stdin: None,
            input_task: None,
            resize_task: None,
            resize_sender: None,
            pending_start: None,
            cursor_visible: true,
        };
        tab.output(&format!(
            "\x1b[2mStarting {} on {}\x1b[0m\r\n\r\n",
            tab.command.join(" "),
            request.confirmation.target
        ));
        tab.pending_start = Some(request);
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
                                tab.refresh_view();
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
        let Some(mut request) = tab.pending_start.take() else {
            return;
        };
        let (rows, cols) = tab.parser.screen().size();
        request.terminal_size = PodExecTerminalSize {
            cols: cols.get(),
            rows: rows.get(),
        };
        let read = self.backend.exec(request);
        tab.task = Some(cx.spawn(async move |this, cx| match read.await {
            Ok(mut stream) => {
                let id = stream.summary.id.clone();
                let alive = this
                    .update(cx, |this, cx| {
                        let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) else {
                            return false;
                        };
                        tab.id = Some(id);
                        tab.status =
                            crate::live_sessions::exec_status(stream.summary.status).into();
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
                        tab.id = None;
                        tab.stdin = None;
                        tab.input_task = None;
                        if matches!(tab.status.as_str(), "starting" | "connecting" | "running") {
                            tab.status = "stopped".into();
                        }
                    }
                    this.load_sessions(cx);
                    cx.notify();
                });
            }
            Err(error) => {
                let _ = this.update(cx, |this, cx| {
                    if let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) {
                        tab.status = "error".into();
                        tab.error = Some(error.message);
                    }
                    cx.notify();
                });
            }
        }));
        cx.notify();
    }

    pub(super) fn exec_input(&mut self, data: String, cx: &mut Context<Self>) {
        let Some(tab) = self.exec.active() else {
            return;
        };
        if let Some(sender) = &tab.stdin {
            let _ = sender.send(data);
        }
        if let Some(tab) = self
            .exec
            .tabs
            .iter_mut()
            .find(|t| Some(t.key) == self.exec.active)
        {
            tab.parser.screen_mut().set_scrollback(0);
            tab.selection = None;
            tab.refresh_view();
        }
        cx.notify();
    }
    fn open_exec_input(&mut self, key: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.exec.tabs.iter_mut().find(|t| t.key == key) else {
            return;
        };
        let Some(id) = tab.id.clone() else {
            return;
        };
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        tab.stdin = Some(sender);
        let backend = self.backend.clone();
        tab.input_task = Some(cx.spawn(async move |this, cx| {
            while let Some(data) = receiver.recv().await {
                if let Err(error) = backend.exec_input(id.clone(), data).await {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) {
                            tab.error = Some(error.message);
                        }
                        cx.notify();
                    });
                    break;
                }
            }
        }));
    }
    pub(super) fn stop_exec_tab(&mut self, key: usize, cx: &mut Context<Self>) {
        let Some(id) = self
            .exec
            .tabs
            .iter()
            .find(|t| t.key == key)
            .and_then(|t| t.id.clone())
        else {
            return;
        };
        let read = self.backend.stop_exec(id);
        cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this.exec.tabs.iter_mut().find(|t| t.key == key) {
                    match result {
                        Ok(true) => {
                            tab.task.take();
                            tab.input_task.take();
                            tab.stdin.take();
                            tab.id = None;
                            tab.status = "stopped".into();
                        }
                        Ok(false) => {
                            tab.error = Some("Stop failed; the session may still be running".into())
                        }
                        Err(error) => tab.error = Some(error.message),
                    }
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
