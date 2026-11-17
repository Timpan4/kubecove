//! Each surface follows the reads and watches for the evidence it shows.
use crate::inspector::{Inspector, map::TopologyLoad, watch_status::LiveStatus};

pub(in crate::inspector) struct NoticeFreshness {
    pub status: LiveStatus,
    pub map_reload_failed: bool,
}

impl Inspector {
    pub(in crate::inspector) fn table_updates_current(&self) -> bool {
        !self.observation.shown_loading()
            && self.live.refresh_error.is_none()
            && self
                .live
                .watch_status_where(|context| self.observation.shows(context))
                == LiveStatus::Live
    }

    pub(in crate::inspector) fn map_updates_current(&self) -> bool {
        self.shown_map_context().is_some_and(|map_context| {
            self.map.read.error.is_none()
                && self
                    .live
                    .watch_status_where(|context| context == map_context)
                    == LiveStatus::Live
        })
    }

    pub(in crate::inspector) fn notice_freshness(&self) -> NoticeFreshness {
        let map_context = self.shown_map_context();
        let status = if self.observation.refreshing() {
            LiveStatus::Refreshing
        } else if self.live.refresh_error.is_some() {
            LiveStatus::RefreshFailed
        } else {
            self.live.watch_status_where(|context| {
                (self.table_open && self.observation.shows(context))
                    || map_context.as_deref() == Some(context)
            })
        };
        NoticeFreshness {
            status,
            map_reload_failed: map_context.is_some() && self.map.read.error.is_some(),
        }
    }

    fn shown_map_context(&self) -> Option<String> {
        (self.map_is_open() && matches!(self.map.topology, TopologyLoad::Ready(_)))
            .then(|| self.map_context())
    }
}
