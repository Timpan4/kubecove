//! Log stream state of the resource detail panel, mirroring
//! `ResourceLogsPane.svelte`: one stream per target and options, batched
//! appends, pause buffering, follow-to-latest and the unseen-line count.

use super::Inspector;
use super::detail::Detail;
use crate::backend::{LogStream, LogTarget};
use crate::{log_buffer::LogBuffer, logs::MAX_RETAINED_LOG_LINES};
use gpui_kit::component::{
    IndexPath,
    input::{InputEvent, InputState},
    select::{SearchableVec, SelectEvent, SelectState},
};
use gpui_kit::{
    AppContext, Context, Entity, FollowMode, ListAlignment, ListOffset, ListState, SharedString,
    Subscription, Task, Window, px,
};
use kubecove_lib::models::{
    AggregatedLogStreamRequest, LogLineSource, PodLogStreamRequest, ResourceDetailsFull,
    StreamMessage, StreamStatus,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LogStatus {
    Idle,
    Connecting,
    Connected,
    Reconnecting,
    Forbidden,
    Error,
    Stopped,
}

/// `sinceOptions`.
pub(super) const SINCE: &[(Option<i64>, &str, &str)] = &[
    (None, "Any time", "Any"),
    (Some(300), "5m", "5m"),
    (Some(900), "15m", "15m"),
    (Some(3600), "1h", "1h"),
];

pub(super) struct LogsState {
    pub buffer: LogBuffer,
    /// Arrival indices in display order after the text and level filters.
    pub visible: Vec<u64>,
    /// The previous `visible`, which `refilter` diffs against and reuses.
    previous: Vec<u64>,
    pub list: ListState,
    pub status: LogStatus,
    pub message: String,
    pub error: Option<String>,
    pub filter: Entity<InputState>,
    /// `logFilterTerm`: the trimmed, lowercased filter.
    pub term: String,
    pub hidden: [bool; 4],
    pub wrap: bool,
    pub latest_first: bool,
    /// `buffer.has_levels()` when the rows were last measured. The level
    /// column narrows every row, so a change remeasures the kept rows.
    levels: bool,
    /// `expandAll`: one-line rows open unless the user toggled them away.
    pub expand_all: bool,
    /// Buffer indices whose state differs from `expand_all` (`toggledRows`).
    toggled: HashSet<u64>,
    /// The buffer index whose Copy line button last copied its raw text.
    pub copied_line: Option<u64>,
    pub row_focus: RefCell<HashMap<u64, gpui_kit::FocusHandle>>,
    pub paused: bool,
    pending: VecDeque<(String, Option<LogLineSource>)>,
    pub unseen: usize,
    pub tail_lines: i64,
    pub since: Option<i64>,
    pub tail_menu_open: bool,
    pub tail_input: Entity<InputState>,
    pub containers: Vec<String>,
    pub container: Option<String>,
    pub container_picker: Option<Entity<SelectState<SearchableVec<SharedString>>>>,
    aggregate: bool,
    task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl LogsState {
    pub fn new(
        data: &ResourceDetailsFull,
        window: &mut Window,
        cx: &mut Context<Inspector>,
    ) -> Self {
        let summary = &data.summary;
        let is_pod = summary.kind == "Pod";
        let containers = if is_pod {
            crate::logs::container_options(data.status.as_ref())
        } else {
            Vec::new()
        };
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter logs..."));
        let tail_input = cx.new(|cx| InputState::new(window, cx).default_value("200"));
        let mut subscriptions = vec![
            cx.subscribe_in(&filter, window, |this, state, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let term = state.read(cx).value().trim().to_lowercase();
                    if let Some(logs) = this.logs_state() {
                        logs.term = term;
                        logs.refilter();
                    }
                    cx.notify();
                }
            }),
            cx.subscribe_in(&tail_input, window, |this, state, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    // `setTailLines`: non-numeric input reads as zero.
                    let value = state.read(cx).value().trim().parse::<f64>().unwrap_or(0.);
                    let lines = if value.is_finite() {
                        value.max(0.).floor() as i64
                    } else {
                        0
                    };
                    if let Some(logs) = this.logs_state()
                        && logs.tail_lines != lines
                    {
                        logs.tail_lines = lines;
                        this.restart_logs(window, cx);
                    }
                }
            }),
        ];
        let container_picker = (!containers.is_empty()).then(|| {
            let items: Vec<SharedString> = containers.iter().cloned().map(Into::into).collect();
            let picker = cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(items),
                    Some(IndexPath::new(0)),
                    window,
                    cx,
                )
            });
            subscriptions.push(
                cx.subscribe_in(&picker, window, |this, _, event, window, cx| {
                    let SelectEvent::Confirm(Some(name)) = event else {
                        return;
                    };
                    if let Some(logs) = this.logs_state()
                        && logs.container.as_deref() != Some(name.as_ref())
                    {
                        logs.container = Some(name.to_string());
                        this.restart_logs(window, cx);
                    }
                }),
            );
            picker
        });
        Self {
            buffer: LogBuffer::default(),
            visible: Vec::new(),
            previous: Vec::new(),
            list: follow_list(false),
            status: LogStatus::Idle,
            message: "Log stream idle".into(),
            error: None,
            filter,
            term: String::new(),
            hidden: [false; 4],
            wrap: true,
            latest_first: false,
            levels: false,
            expand_all: false,
            toggled: HashSet::new(),
            copied_line: None,
            row_focus: RefCell::new(HashMap::new()),
            paused: false,
            pending: VecDeque::new(),
            unseen: 0,
            tail_lines: 200,
            since: None,
            tail_menu_open: false,
            tail_input,
            container: containers.first().cloned(),
            containers,
            container_picker,
            aggregate: matches!(summary.kind.as_str(), "Deployment" | "Service"),
            task: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn is_aggregate(&self) -> bool {
        self.aggregate
    }

    pub fn connecting(&self) -> bool {
        !self.paused && matches!(self.status, LogStatus::Connecting | LogStatus::Reconnecting)
    }

    /// `statusLabel`.
    pub fn status_label(&self) -> String {
        if self.paused {
            return format!("Paused · {} buffered", self.pending.len());
        }
        match self.status {
            LogStatus::Connected => "Live",
            LogStatus::Idle => "Idle",
            LogStatus::Connecting => "Connecting",
            LogStatus::Reconnecting => "Reconnecting",
            LogStatus::Forbidden => "Forbidden",
            LogStatus::Error => "Error",
            LogStatus::Stopped => "Stopped",
        }
        .into()
    }

    /// `tailLabel`.
    pub fn tail_label(&self) -> String {
        let since = SINCE
            .iter()
            .find(|(seconds, ..)| *seconds == self.since)
            .map_or("Any time", |(_, label, _)| label);
        if self.tail_lines > 0 {
            format!("Last {} · {since}", self.tail_lines)
        } else {
            format!("New only · {since}")
        }
    }

    pub fn filtering(&self) -> bool {
        !self.term.is_empty() || self.hidden.iter().any(|hidden| *hidden)
    }

    /// Recomputes the visible rows and tells the list what changed. Appends
    /// and evictions splice; other changes remeasure.
    pub fn refilter(&mut self) {
        let pinned = self.pinned();
        let levels = self.buffer.has_levels();
        let levels_changed = levels != self.levels;
        self.levels = levels;
        // The two index lists swap roles, so a refilter allocates only when
        // the visible rows outgrow both.
        std::mem::swap(&mut self.visible, &mut self.previous);
        self.buffer.visible(
            &self.term,
            self.hidden,
            self.latest_first,
            &mut self.visible,
        );
        let previous = &self.previous;
        let next = &self.visible;
        // Evicted lines leave the oldest end; new lines arrive at the newest end.
        let first = self.buffer.first_index();
        self.row_focus.get_mut().retain(|index, _| *index >= first);
        let evicted = previous.iter().filter(|index| **index < first).count();
        if !self.latest_first {
            let kept = &previous[evicted..];
            if next.starts_with(kept) {
                self.list.splice(0..evicted, 0);
                self.list
                    .splice(kept.len()..kept.len(), next.len() - kept.len());
                if levels_changed {
                    self.list.remeasure();
                }
                return;
            }
        } else {
            let kept = &previous[..previous.len() - evicted];
            if next.ends_with(kept) {
                let kept_len = kept.len();
                self.list.splice(kept_len..previous.len(), 0);
                self.list.splice(0..0, next.len() - kept_len);
                if levels_changed {
                    self.list.remeasure();
                }
                if pinned {
                    self.list.scroll_to(ListOffset::default());
                }
                return;
            }
        }
        self.list.reset(next.len());
        if pinned {
            self.jump_to_latest();
        }
    }

    /// Whether a one-line row shows its details. `LogsTab.svelte` clears the
    /// toggled set on Expand all and on Clear, and keeps `expandAll` otherwise.
    pub fn row_open(&self, index: u64) -> bool {
        self.expand_all != self.toggled.contains(&index)
    }

    pub fn toggle_row(&mut self, index: u64) {
        if !self.toggled.remove(&index) {
            self.toggled.insert(index);
        }
    }

    pub fn toggle_expand_all(&mut self) {
        self.expand_all = !self.expand_all;
        self.toggled.clear();
    }

    /// `onClear` and `resetLogLines`.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.row_focus.get_mut().clear();
        self.pending.clear();
        self.unseen = 0;
        self.visible.clear();
        self.toggled.clear();
        self.copied_line = None;
        self.list.reset(0);
        if !self.latest_first {
            self.list.set_follow_mode(FollowMode::Tail);
        }
    }

    fn append(&mut self, batch: Vec<(String, Option<LogLineSource>)>) {
        if batch.is_empty() {
            return;
        }
        if self.paused {
            self.pending.extend(batch);
            while self.pending.len() > MAX_RETAINED_LOG_LINES {
                self.pending.pop_front();
            }
            return;
        }
        let following = self.pinned();
        let added = self.buffer.append(batch);
        self.toggled
            .retain(|index| self.buffer.line(*index).is_some());
        // Evictions shift buffer positions, so `refilter` remeasures then.
        self.refilter();
        if !following {
            self.unseen += added;
        }
    }

    /// Whether the view shows the latest line, as `pinnedToLatest` tracks.
    pub fn pinned(&self) -> bool {
        if self.latest_first {
            self.list.logical_scroll_top() == ListOffset::default()
        } else {
            self.list.is_following_tail() || self.visible.is_empty()
        }
    }

    /// `togglePause`.
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if !self.paused {
            let pending = Vec::from(std::mem::take(&mut self.pending));
            self.append(pending);
        }
    }

    /// `jumpToLatest`.
    pub fn jump_to_latest(&mut self) {
        self.unseen = 0;
        if self.latest_first {
            self.list.scroll_to(ListOffset::default());
        } else {
            self.list.set_follow_mode(FollowMode::Tail);
        }
    }

    pub fn set_latest_first(&mut self, latest_first: bool) {
        self.latest_first = latest_first;
        self.list = follow_list(latest_first);
        // The new list has no rows, so `refilter` must not diff against the
        // old ones: one unchanged row would otherwise splice nothing in.
        self.visible.clear();
        self.refilter();
        self.jump_to_latest();
    }

    fn target(&self, data: &ResourceDetailsFull) -> Option<LogTarget> {
        let summary = &data.summary;
        let namespace = summary.namespace.clone()?;
        let tail_lines = Some(self.tail_lines);
        if summary.kind == "Pod" {
            return Some(LogTarget::Pod(PodLogStreamRequest {
                batch_lines: true,
                cluster_context: summary.cluster.clone(),
                kubeconfig_env_var: None,
                namespace,
                pod_name: summary.name.clone(),
                container: Some(self.container.clone()?),
                tail_lines,
                since_seconds: self.since,
            }));
        }
        self.aggregate.then(|| {
            LogTarget::Aggregate(AggregatedLogStreamRequest {
                batch_lines: true,
                cluster_context: summary.cluster.clone(),
                kubeconfig_env_var: None,
                namespace,
                target_kind: summary.kind.clone(),
                target_name: summary.name.clone(),
                tail_lines,
                since_seconds: self.since,
            })
        })
    }

    fn receive(&mut self, messages: Vec<StreamMessage>) {
        let mut lines = Vec::new();
        for message in messages {
            match message {
                StreamMessage::Status {
                    status, message, ..
                } => {
                    self.status = match status {
                        StreamStatus::Connected => LogStatus::Connected,
                        StreamStatus::Reconnecting => LogStatus::Reconnecting,
                        StreamStatus::Forbidden => LogStatus::Forbidden,
                    };
                    self.message = message;
                }
                StreamMessage::LogLine { line, source, .. } => {
                    self.connected();
                    lines.push((line, source));
                }
                StreamMessage::LogLines {
                    lines: batch,
                    source,
                    ..
                } => {
                    self.connected();
                    lines.extend(batch.into_iter().map(|line| (line, source.clone())));
                }
                StreamMessage::Error { message, .. } => {
                    self.status = LogStatus::Error;
                    self.message = "Log stream error".into();
                    self.error = Some(message);
                }
                StreamMessage::Stopped { .. } => {
                    self.status = LogStatus::Stopped;
                    self.message = "Log stream stopped".into();
                }
                StreamMessage::Started { .. }
                | StreamMessage::ResourceChanged { .. }
                | StreamMessage::ResourceEventsChanged { .. } => {}
            }
        }
        self.append(lines);
    }

    fn connected(&mut self) {
        self.status = LogStatus::Connected;
        self.message = if self.aggregate {
            "Aggregated logs connected"
        } else {
            "Log stream connected"
        }
        .into();
    }
}

fn follow_list(latest_first: bool) -> ListState {
    let list = ListState::new(0, ListAlignment::Top, px(400.));
    if !latest_first {
        list.set_follow_mode(FollowMode::Tail);
    }
    list
}

impl Inspector {
    pub(super) fn logs_state(&mut self) -> Option<&mut LogsState> {
        match &mut self.detail {
            Detail::Ready(detail) => detail.logs.as_mut(),
            _ => None,
        }
    }

    /// Starts the stream the first time the tab opens, and again on
    /// re-entry after it stopped or failed.
    pub(super) fn open_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        if detail.logs.is_none() {
            detail.logs = Some(LogsState::new(&detail.data, window, cx));
        }
        let logs = detail.logs.as_ref().expect("logs state was just created");
        if logs.task.is_none() || matches!(logs.status, LogStatus::Stopped | LogStatus::Error) {
            self.restart_logs(window, cx);
        }
    }

    /// A new target or option resets the lines and opens a new stream; the
    /// old stream stops when its task drops.
    pub(super) fn restart_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        let target = detail
            .logs
            .as_ref()
            .and_then(|logs| logs.target(&detail.data));
        let Some(logs) = detail.logs.as_mut() else {
            return;
        };
        logs.task = None;
        logs.clear();
        logs.error = None;
        let Some(target) = target else {
            logs.status = LogStatus::Idle;
            logs.message = "Log stream idle".into();
            cx.notify();
            return;
        };
        logs.status = LogStatus::Connecting;
        logs.message = "Starting log stream".into();
        let read = backend.log_stream(target);
        let read = self.timings.measure("log_stream", read);
        logs.task = Some(cx.spawn_in(window, async move |this, cx| {
            let mut stream: LogStream = match read.await {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(logs) = this.logs_state() {
                            logs.status = LogStatus::Error;
                            logs.message = "Log stream failed".into();
                            logs.error = Some(error.message);
                        }
                        cx.notify();
                    });
                    return;
                }
            };
            // One update per wake-up, like the web's per-frame flush.
            while let Some(first) = stream.messages.recv().await {
                let mut batch = vec![first];
                while let Ok(message) = stream.messages.try_recv() {
                    batch.push(message);
                }
                let updated = this.update(cx, |this, cx| {
                    if let Some(logs) = this.logs_state() {
                        logs.receive(batch);
                    }
                    cx.notify();
                });
                if updated.is_err() {
                    return;
                }
            }
        }));
        cx.notify();
    }
}
