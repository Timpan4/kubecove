//! Each surface follows the reads and watches for the evidence it shows.
use crate::inspector::{Inspector, map::TopologyLoad, watch_status::LiveStatus};

pub(in crate::inspector) struct NoticeFreshness {
    pub table: Option<LiveStatus>,
    pub map: Option<LiveStatus>,
    pub map_watch: Option<LiveStatus>,
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

    pub(in crate::inspector) fn visible_refresh_error(&self) -> Option<&str> {
        if self.table_open
            && let Some(error) = self.table_refresh_error()
        {
            return Some(error);
        }
        if self.map_is_open()
            && let TopologyLoad::Ready(topology) = &self.map.topology
        {
            return self
                .observation
                .map_rows_error(self.map_context(), topology);
        }
        self.live.refresh_error.as_deref()
    }

    pub(in crate::inspector) fn notice_read_errors(
        &self,
        freshness: &NoticeFreshness,
    ) -> (Option<&str>, Option<&str>) {
        let table = if freshness.table == Some(LiveStatus::RefreshFailed) {
            self.table_refresh_error().or_else(|| {
                self.observation
                    .contexts()
                    .iter()
                    .filter(|context| self.observation.shows(context))
                    .find_map(|context| self.live.reload_error_in(context))
            })
        } else {
            None
        };
        let map = if freshness.map.is_some() {
            self.map_read_error()
        } else {
            None
        };
        (table, map)
    }

    fn map_read_error(&self) -> Option<&str> {
        let TopologyLoad::Ready(topology) = &self.map.topology else {
            return None;
        };
        let context = self.map_context();
        self.observation
            .map_rows_error(context, topology)
            .or_else(|| self.live.reload_error_in(context))
    }

    pub(in crate::inspector) fn notice_freshness(&self) -> NoticeFreshness {
        let table = (self.table_open && self.observation.any_shown()).then(|| self.table_status());
        let mut map_watch = None;
        let map = match &self.map.topology {
            TopologyLoad::Ready(topology) if self.map_is_open() => Some({
                if self.map.read.error.is_some() {
                    LiveStatus::RefreshFailed
                } else {
                    let context = self.map_context();
                    let status = self.live.watch_status_where(|watched| watched == context);
                    map_watch = Some(status);
                    // The graph is filtered by this context's rows.
                    if status == LiveStatus::RefreshFailed
                        || (status == LiveStatus::Live
                            && (self.map.read.refreshing()
                                || self.observation.context_rows_loading(context)
                                || self.observation.map_rows_error(context, topology).is_some()))
                    {
                        LiveStatus::CatchingUp
                    } else {
                        status
                    }
                }
            }),
            _ => None,
        };
        NoticeFreshness {
            table,
            map,
            map_watch,
        }
    }
}
