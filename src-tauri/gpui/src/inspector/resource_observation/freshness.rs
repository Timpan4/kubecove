//! Each surface follows the reads and watches for the evidence it shows.
use crate::inspector::{Inspector, map::TopologyLoad, watch_status::LiveStatus};

pub(in crate::inspector) struct NoticeFreshness {
    pub table: Option<LiveStatus>,
    pub map: Option<LiveStatus>,
}

impl Inspector {
    pub(in crate::inspector) fn table_updates_current(&self) -> bool {
        self.table_status() == LiveStatus::Live
    }

    fn table_status(&self) -> LiveStatus {
        if self.observation.refreshing() {
            LiveStatus::Refreshing
        } else if self.table_refresh_error().is_some() {
            LiveStatus::RefreshFailed
        } else {
            let status = self
                .live
                .watch_status_where(|context| self.observation.shows(context));
            if status == LiveStatus::Live && self.observation.shown_loading() {
                LiveStatus::CatchingUp
            } else {
                status
            }
        }
    }

    pub(in crate::inspector) fn table_refresh_error(&self) -> Option<&str> {
        self.live
            .refresh_error
            .as_deref()
            .or_else(|| self.observation.shown_refresh_error())
    }

    pub(in crate::inspector) fn notice_freshness(&self) -> NoticeFreshness {
        let table = (self.table_open && self.observation.any_shown()).then(|| self.table_status());
        let map = (self.map_is_open() && matches!(self.map.topology, TopologyLoad::Ready(_))).then(
            || {
                if self.map.read.error.is_some() {
                    LiveStatus::RefreshFailed
                } else {
                    let context = self.map_context();
                    let status = self.live.watch_status_where(|watched| watched == context);
                    // The graph is filtered by this context's rows.
                    if status == LiveStatus::RefreshFailed
                        || (status == LiveStatus::Live
                            && (self.map.read.refreshing()
                                || self.observation.context_rows_stale(context)))
                    {
                        LiveStatus::CatchingUp
                    } else {
                        status
                    }
                }
            },
        );
        NoticeFreshness { table, map }
    }
}
