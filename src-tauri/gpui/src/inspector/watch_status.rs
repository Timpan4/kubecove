//! Live watch status: what each watch's stream reported, and the one label the
//! resource and GitOps views derive from a set of watches (`resource-watch.ts`,
//! `ResourceLiveStatusMenu.svelte`).
use kubecove_lib::models::StreamStatus;

/// The live-updates label. Both views read it through [`aggregate`], so the
/// precedence lives in one place and callers compare variants, not text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LiveStatus {
    NotConnected,
    Connecting,
    Live,
    LiveLimited,
    PollingLimited,
    CatchingUp,
    Reconnecting,
    RefreshFailed,
    Refreshing,
}

impl LiveStatus {
    pub(super) fn label(self) -> &'static str {
        match self {
            LiveStatus::NotConnected => "Not connected",
            LiveStatus::Connecting => "Connecting",
            LiveStatus::Live => "Live",
            LiveStatus::LiveLimited => "Live (limited)",
            LiveStatus::PollingLimited => "Polling (limited)",
            LiveStatus::CatchingUp => "Catching up",
            LiveStatus::Reconnecting => "Reconnecting",
            LiveStatus::RefreshFailed => "Refresh failed",
            LiveStatus::Refreshing => "Refreshing…",
        }
    }
}

/// One watch's connection, as [`aggregate`] weighs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WatchLink {
    /// Not started, or started with no status or error reported yet.
    Opening,
    Connected,
    /// Watch access is denied; readable rows refresh on the recovery tick.
    Denied,
    /// The stream failed or dropped and recovers on the next tick.
    Lost,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WatchSummary {
    pub link: WatchLink,
    /// The watch's last row reload failed.
    pub refresh_failed: bool,
    /// The watch's rows reload after a notice that did not name them.
    pub catching_up: bool,
}

impl From<WatchLink> for WatchSummary {
    fn from(link: WatchLink) -> Self {
        Self {
            link,
            refresh_failed: false,
            catching_up: false,
        }
    }
}

/// What a set of watches reports together: `status` in `resource-watch.ts`,
/// and `IDLE_LIVE_VIEW` in `live-view-refresh.ts` for a view without watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Connection {
    Idle,
    /// A watch has not connected yet, and none was lost.
    Connecting,
    /// A watch that was connected, or failed to open, is recovering.
    Reconnecting,
    /// Every watch is denied.
    Polling,
    /// Some watches are denied and the rest are connected.
    Partial,
    Connected,
}

impl Connection {
    pub(super) fn of(watches: impl IntoIterator<Item = WatchLink>) -> Self {
        let (mut any, mut lost, mut opening, mut denied, mut connected) =
            (false, false, false, false, false);
        for link in watches {
            any = true;
            match link {
                WatchLink::Lost => lost = true,
                WatchLink::Opening => opening = true,
                WatchLink::Denied => denied = true,
                WatchLink::Connected => connected = true,
            }
        }
        if !any {
            Connection::Idle
        } else if lost {
            Connection::Reconnecting
        } else if opening {
            Connection::Connecting
        } else if !connected {
            Connection::Polling
        } else if denied {
            Connection::Partial
        } else {
            Connection::Connected
        }
    }

    /// The status menu's message for this state.
    pub(super) fn message(self) -> &'static str {
        match self {
            Connection::Idle => "No live resource watches in this view.",
            Connection::Connecting => "Starting live updates",
            Connection::Reconnecting => "Reconnecting; periodically reloading this view",
            Connection::Polling => {
                "Watch access is unavailable. Readable resources refresh periodically."
            }
            Connection::Partial => {
                "Live updates connected for permitted resources. Readable resources without watch access refresh periodically."
            }
            Connection::Connected => "Live updates connected",
        }
    }

    /// `STATUS_SEVERITY` in `live-view-refresh.ts`: the lowest rank names a
    /// view of several contexts.
    pub(super) fn severity(self) -> u8 {
        match self {
            Connection::Reconnecting => 0,
            Connection::Connecting => 1,
            Connection::Partial => 2,
            Connection::Polling => 3,
            Connection::Connected => 4,
            Connection::Idle => 5,
        }
    }
}

/// The label for a view's watches. `refresh_failed` is the view's own read.
pub(super) fn aggregate(
    refresh_failed: bool,
    watches: impl IntoIterator<Item = WatchSummary>,
) -> LiveStatus {
    if refresh_failed {
        return LiveStatus::RefreshFailed;
    }
    let (mut reload_failed, mut catching_up) = (false, false);
    let connection = Connection::of(watches.into_iter().map(|watch| {
        reload_failed |= watch.refresh_failed;
        catching_up |= watch.catching_up;
        watch.link
    }));
    match connection {
        Connection::Idle => LiveStatus::NotConnected,
        _ if reload_failed => LiveStatus::RefreshFailed,
        Connection::Reconnecting => LiveStatus::Reconnecting,
        _ if catching_up => LiveStatus::CatchingUp,
        Connection::Connecting => LiveStatus::Connecting,
        Connection::Polling => LiveStatus::PollingLimited,
        Connection::Partial => LiveStatus::LiveLimited,
        Connection::Connected => LiveStatus::Live,
    }
}

/// What a running resource watch's stream last reported.
pub(super) enum Link {
    Opening,
    Connected,
    Denied(String),
    /// The last error, if any, and a denial reported before it, both still listed.
    Lost {
        error: Option<String>,
        denied: Option<String>,
    },
}

impl Link {
    pub(super) fn status(&mut self, status: StreamStatus, message: String) {
        *self = match status {
            StreamStatus::Connected => Link::Connected,
            StreamStatus::Forbidden => Link::Denied(message),
            _ => Link::Lost {
                error: self.take_error(),
                denied: None,
            },
        };
    }

    pub(super) fn failed(&mut self, message: String) {
        let denied = match std::mem::replace(self, Link::Opening) {
            Link::Denied(denied) => Some(denied),
            Link::Lost { denied, .. } => denied,
            Link::Opening | Link::Connected => None,
        };
        *self = Link::Lost {
            error: Some(message),
            denied,
        };
    }

    /// The stream ended. A denied or opening watch keeps its state until the next status.
    pub(super) fn stopped(&mut self) {
        if matches!(self, Link::Connected) {
            *self = Link::Lost {
                error: None,
                denied: None,
            };
        }
    }

    fn take_error(&mut self) -> Option<String> {
        match self {
            Link::Lost { error, .. } => error.take(),
            _ => None,
        }
    }

    pub(super) fn summary(&self) -> WatchLink {
        match self {
            Link::Opening => WatchLink::Opening,
            Link::Connected => WatchLink::Connected,
            Link::Denied(_) => WatchLink::Denied,
            Link::Lost { .. } => WatchLink::Lost,
        }
    }

    /// The error, then the denial, as the status menu lists them.
    pub(super) fn messages(&self) -> [Option<&String>; 2] {
        match self {
            Link::Opening | Link::Connected => [None, None],
            Link::Denied(denied) => [None, Some(denied)],
            Link::Lost { error, denied } => [error.as_ref(), denied.as_ref()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LiveStatus, WatchLink, WatchSummary, aggregate};
    use WatchLink::{Connected, Denied, Lost, Opening};

    fn watch(link: WatchLink) -> WatchSummary {
        link.into()
    }

    #[test]
    fn precedence_runs_from_a_failed_view_read_down_to_live() {
        let failed_reload = WatchSummary {
            refresh_failed: true,
            ..watch(Connected)
        };
        let catching_up = WatchSummary {
            catching_up: true,
            ..watch(Connected)
        };
        let cases: [(bool, Vec<WatchSummary>, LiveStatus); 12] = [
            (true, vec![watch(Opening)], LiveStatus::RefreshFailed),
            (false, vec![], LiveStatus::NotConnected),
            (true, vec![], LiveStatus::RefreshFailed),
            (
                false,
                vec![watch(Opening), watch(Opening)],
                LiveStatus::Connecting,
            ),
            (
                false,
                vec![failed_reload, watch(Lost)],
                LiveStatus::RefreshFailed,
            ),
            // A watch that has not connected yet was not lost, so the view connects.
            (
                false,
                vec![watch(Opening), watch(Connected)],
                LiveStatus::Connecting,
            ),
            (
                false,
                vec![watch(Opening), watch(Lost), watch(Connected)],
                LiveStatus::Reconnecting,
            ),
            (
                false,
                vec![watch(Lost), catching_up, watch(Denied)],
                LiveStatus::Reconnecting,
            ),
            (
                false,
                vec![catching_up, watch(Denied)],
                LiveStatus::CatchingUp,
            ),
            (
                false,
                vec![watch(Denied), watch(Denied)],
                LiveStatus::PollingLimited,
            ),
            (
                false,
                vec![watch(Denied), watch(Connected)],
                LiveStatus::LiveLimited,
            ),
            (
                false,
                vec![watch(Connected), watch(Connected)],
                LiveStatus::Live,
            ),
        ];
        for (view_failed, watches, expected) in cases {
            assert_eq!(
                aggregate(view_failed, watches.clone()),
                expected,
                "{watches:?}"
            );
        }
    }
}
