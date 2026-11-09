//! The resource scope read and what the table shows for it
//! (`state-machines.md` #5). The web keeps this in its scope and custom-kind
//! queries; here one owner holds the read so the table cannot wait on a read
//! that no longer runs.
use super::{
    Inspector, Load, custom_resources::CustomLoad, request::InFlight, watch_status::LiveStatus,
};
use gpui_kit::Task;

pub(super) struct ScopeLoad {
    /// What the table shows for the selected context.
    pub(super) rows: Load,
    /// The last settled custom-kind read. `custom` adds the pending one.
    custom: CustomLoad,
    read: Option<ScopeRead>,
}

/// The scope read in flight. Dropping it cancels the read.
enum ScopeRead {
    /// The built-in read of `load_scope`. With `custom`, the same task then
    /// reads the custom kinds.
    Builtin { custom: bool, _task: InFlight },
    /// The built-ins applied; the task reads the custom kinds.
    Custom { _task: InFlight },
    /// The live menu's Refresh (clear cache): built-ins and custom kinds in one read.
    Refresh { _task: InFlight },
}

impl Default for ScopeLoad {
    fn default() -> Self {
        Self {
            rows: Load::Loading,
            custom: CustomLoad::Idle,
            read: None,
        }
    }
}

impl ScopeLoad {
    /// Starts `load_scope`'s read, replacing and so cancelling any scope read.
    pub(super) fn start(&mut self, custom: bool, task: Task<()>) {
        self.custom = CustomLoad::Idle;
        self.read = Some(ScopeRead::Builtin {
            custom,
            _task: task.into(),
        });
    }

    /// Starts the live menu's refresh, replacing and so cancelling any scope read.
    pub(super) fn start_refresh(&mut self, task: Task<()>) {
        self.read = Some(ScopeRead::Refresh {
            _task: task.into(),
        });
    }

    /// Drops the scope read, which cancels it.
    pub(super) fn cancel(&mut self) {
        self.read = None;
    }

    /// Applies the built-in read. After a success the task goes on to the
    /// custom kinds when it reads them.
    pub(super) fn builtin_settled(&mut self, result: Result<(), String>) {
        let read = self.read.take();
        match result {
            Ok(()) => {
                self.rows = Load::Ready;
                if let Some(ScopeRead::Builtin { custom: true, _task }) = read {
                    self.read = Some(ScopeRead::Custom { _task });
                }
            }
            Err(error) => {
                self.rows = Load::Failed(error);
                self.custom = CustomLoad::Idle;
            }
        }
    }

    pub(super) fn custom_settled(&mut self, ok: bool) {
        self.read = None;
        self.custom = if ok {
            CustomLoad::Ready
        } else {
            CustomLoad::Failed
        };
    }

    /// Applies the refresh. A failure keeps rows the table shows; without
    /// rows, as when Refresh is pressed during a first load, it fails the load.
    pub(super) fn refresh_settled(&mut self, result: Result<(), String>) {
        self.read = None;
        match result {
            Ok(()) => {
                self.rows = Load::Ready;
                self.custom = CustomLoad::Ready;
            }
            Err(_) if matches!(self.rows, Load::Ready) => {}
            Err(error) => self.rows = Load::Failed(error),
        }
    }

    /// A built-in read or a refresh runs, so the rows may still change.
    pub(super) fn loading(&self) -> bool {
        matches!(
            self.read,
            Some(ScopeRead::Builtin { .. } | ScopeRead::Refresh { .. })
        )
    }

    pub(super) fn refreshing(&self) -> bool {
        matches!(self.read, Some(ScopeRead::Refresh { .. }))
    }

    pub(super) fn custom(&self) -> CustomLoad {
        match self.read {
            Some(ScopeRead::Builtin { custom: true, .. } | ScopeRead::Custom { .. }) => {
                CustomLoad::Loading
            }
            _ => self.custom,
        }
    }
}

impl Inspector {
    /// The resource view's live status, `Refreshing` while the live menu's refresh runs.
    pub(super) fn resource_live_status(&self) -> LiveStatus {
        if self.scope_load.refreshing() {
            LiveStatus::Refreshing
        } else {
            self.live.settled_status()
        }
    }

    /// Whether row activity may animate: every watch is live and no read is pending.
    /// Mirrors `updatesCurrent` in `ResourceBrowser.svelte`.
    pub(super) fn updates_current(&self) -> bool {
        !self.scope_load.loading() && self.live.settled_status() == LiveStatus::Live
    }
}
