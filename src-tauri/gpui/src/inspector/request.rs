//! Request slots (`state-machines.md` P1). A slot that owns its task can only
//! report a read in flight while that read runs, and replacing or dropping the
//! slot cancels the read: `NativeBackend::read` aborts its tokio task on drop.
use gpui_kit::Task;

/// One running request. Dropping it drops the task, which cancels the read.
#[must_use]
pub(super) struct InFlight {
    _task: Task<()>,
}

impl From<Task<()>> for InFlight {
    fn from(task: Task<()>) -> Self {
        Self { _task: task }
    }
}

/// One read whose result the view shows. `Loading` owns the read, so assigning
/// a new state cancels the old read.
pub(super) enum Fetch<T> {
    /// No read has started.
    Idle,
    Loading {
        _read: InFlight,
    },
    Ready(T),
    Failed(String),
}

impl<T> Fetch<T> {
    /// A read that owns `read`, so replacing this state cancels it.
    pub(super) fn start(read: Task<()>) -> Self {
        Fetch::Loading { _read: read.into() }
    }

    pub(super) fn loading(&self) -> bool {
        matches!(self, Fetch::Loading { .. })
    }

    pub(super) fn failure(&self) -> Option<&str> {
        match self {
            Fetch::Failed(message) => Some(message),
            _ => None,
        }
    }
}
