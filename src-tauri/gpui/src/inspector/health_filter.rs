//! Resource health filter, mirroring `HealthFilter` and `classifyResourceHealth`
//! in `src/features/resources/helpers.ts`. The summary counts stay on the rows
//! in scope before this filter, as `buildResourceHealthSummary` does on the web.

use super::resource_filters::HealthFilter;
use super::{Inspector, Page};
use crate::incidents::Filter as IncidentFilter;
use gpui_kit::{Context, Window};

impl HealthFilter {
    /// `incidentResourcesHealthFilter` in `src/features/incidents/model.ts`.
    pub(super) fn from_incident(filter: IncidentFilter) -> Self {
        match filter {
            IncidentFilter::Degraded => Self::Degraded,
            IncidentFilter::Attention => Self::Attention,
            IncidentFilter::Restarted => Self::Restarted,
            IncidentFilter::All | IncidentFilter::Warning => Self::All,
        }
    }
}

impl Inspector {
    /// The incident's "Open resources" action: all namespaces, an empty search,
    /// and the incident's health filter, as `openResources` does on the web.
    pub(super) fn open_incident_resources(
        &mut self,
        filter: IncidentFilter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.defer_exec_leave(
            super::exec_leave::LeaveIntent::IncidentResources(filter),
            cx,
        ) {
            return;
        }
        self.select_namespace(None, window, cx);
        if self.page != Page::Resources {
            return;
        }
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.filters.health = HealthFilter::from_incident(filter);
        self.filter(cx);
    }
}
