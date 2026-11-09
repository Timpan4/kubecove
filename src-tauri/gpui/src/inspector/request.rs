//! Request slots (`state-machines.md` P1). A slot that owns its task can only
//! report a read in flight while that read runs, and replacing or dropping the
//! slot cancels the read: `NativeBackend::read` aborts its tokio task on drop.
use gpui_kit::Task;
use kubecove_lib::models::AppError;

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
#[derive(Default)]
pub(super) enum Fetch<T, E = String> {
    /// No read has started.
    #[default]
    Idle,
    Loading {
        _read: InFlight,
    },
    Ready(T),
    Failed(E),
}

impl<T, E> Fetch<T, E> {
    /// A read that owns `read`, so replacing this state cancels it.
    pub(super) fn start(read: Task<()>) -> Self {
        Fetch::Loading { _read: read.into() }
    }

    pub(super) fn loading(&self) -> bool {
        matches!(self, Fetch::Loading { .. })
    }

    pub(super) fn ready(&self) -> Option<&T> {
        match self {
            Fetch::Ready(value) => Some(value),
            _ => None,
        }
    }
}

impl<T> Fetch<T> {
    pub(super) fn failure(&self) -> Option<&str> {
        match self {
            Fetch::Failed(message) => Some(message),
            _ => None,
        }
    }
}

/// A page read that keeps its data while it reads again, as a TanStack Query
/// object does (`state-machines.md` P2). The first read shows pending, then
/// data or a failure. A later failure keeps the data and reports a refresh error.
pub(super) struct Query<T, E = AppError> {
    /// The last settled result; `None` until the first read settles.
    settled: Option<Result<T, E>>,
    /// The read in flight. Dropping it cancels the read.
    read: Option<InFlight>,
    /// A failed read while `settled` holds data. It stays until the next read settles.
    refresh_error: Option<E>,
}

impl<T, E> Default for Query<T, E> {
    fn default() -> Self {
        Self {
            settled: None,
            read: None,
            refresh_error: None,
        }
    }
}

impl<T, E> Query<T, E> {
    /// Starts a read that owns `read`, replacing and so cancelling any read in
    /// flight. What the page shows stays until the read settles.
    pub(super) fn start(&mut self, read: Task<()>) {
        self.read = Some(read.into());
    }

    /// Drops the read in flight, which cancels it. What the page shows stays.
    pub(super) fn cancel(&mut self) {
        self.read = None;
    }

    /// Applies a read's result. A failure while data is shown becomes the
    /// refresh error; any other result replaces what the page shows.
    pub(super) fn settle(&mut self, result: Result<T, E>) {
        self.read = None;
        match result {
            Err(error) if self.data().is_some() => self.refresh_error = Some(error),
            result => {
                self.refresh_error = None;
                self.settled = Some(result);
            }
        }
    }

    /// Forgets a first-read failure, so the next read shows pending, as
    /// TanStack Query does when a query without data reads again.
    pub(super) fn clear_failure(&mut self) {
        if matches!(self.settled, Some(Err(_))) {
            self.settled = None;
        }
    }

    /// Hides the refresh error until the next read settles.
    pub(super) fn clear_refresh_error(&mut self) {
        self.refresh_error = None;
    }

    pub(super) fn fetching(&self) -> bool {
        self.read.is_some()
    }

    /// No read has settled yet.
    pub(super) fn pending(&self) -> bool {
        self.settled.is_none()
    }

    pub(super) fn settled(&self) -> Option<&Result<T, E>> {
        self.settled.as_ref()
    }

    pub(super) fn data(&self) -> Option<&T> {
        self.settled.as_ref().and_then(|result| result.as_ref().ok())
    }

    /// The failure of a read that left no data to show.
    pub(super) fn failure(&self) -> Option<&E> {
        self.settled.as_ref().and_then(|result| result.as_ref().err())
    }

    pub(super) fn refresh_error(&self) -> Option<&E> {
        self.refresh_error.as_ref()
    }
}
