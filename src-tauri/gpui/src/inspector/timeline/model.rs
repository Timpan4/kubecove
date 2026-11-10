//! Prepared timeline content, separate from expansion and viewport geometry.

use super::super::{
    detail::ResourceDetail,
    events::EventsLoad,
    windowed::{Windowed, row_id},
};
use crate::{
    incident::ConditionRow,
    incident_timeline::{self as entries, TimelineItem},
    preferences::TimestampTimezone,
};
use kubecove_lib::models::{ResourceEventSummary, ResourceSummary};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

#[cfg(test)]
mod tests;

pub(super) struct Row {
    pub item: usize,
    pub key: String,
    pub time: String,
    pub ids: [u64; 2],
}

pub(super) struct Group {
    pub key: Option<String>,
    pub label: String,
    pub rows: Vec<Row>,
}

pub(super) struct Model {
    pub items: Vec<TimelineItem>,
    pub groups: Vec<Group>,
}

impl Model {
    fn new(
        summary: &ResourceSummary,
        conditions: &[ConditionRow],
        events: &[ResourceEventSummary],
        timezone: TimestampTimezone,
    ) -> Self {
        let items = entries::incident_timeline(summary, conditions, events);
        let index: HashMap<_, _> = items
            .iter()
            .enumerate()
            .map(|(i, item)| (item.id.as_str(), i))
            .collect();
        let groups = entries::group_by_day(&items, timezone)
            .into_iter()
            .map(|group| {
                let last = group.items.len().saturating_sub(1);
                let rows = group
                    .items
                    .into_iter()
                    .enumerate()
                    .map(|(i, item)| {
                        let time = if group.key.is_some() {
                            entries::timeline_time(item, timezone)
                        } else {
                            String::new()
                        };
                        let ids = [false, true].map(|open| {
                            row_id((
                                (
                                    &item.id,
                                    item.source,
                                    &item.title,
                                    &item.detail,
                                    &item.timestamp,
                                ),
                                (&time, i == 0, i == last, open, &summary.kind),
                            ))
                        });
                        Row {
                            item: index[item.id.as_str()],
                            key: item.key(),
                            time,
                            ids,
                        }
                    })
                    .collect();
                Group {
                    key: group.key,
                    label: group.label,
                    rows,
                }
            })
            .collect();
        Self { items, groups }
    }
}

struct Current {
    data: Arc<[u8]>,
    events: Arc<[u8]>,
    timezone: TimestampTimezone,
    model: Rc<Model>,
}

#[derive(Default)]
pub(crate) struct State {
    pub(super) rows: Windowed,
    current: RefCell<Option<Current>>,
}

impl State {
    pub(super) fn model(&self, detail: &ResourceDetail, timezone: TimestampTimezone) -> Rc<Model> {
        let mut current = self.current.borrow_mut();
        if !matches!(detail.events.load, EventsLoad::Pending)
            && let Some(previous) = &*current
            && previous.data == detail.timeline_source
            && previous.events == detail.events.layout_source
            && previous.timezone == timezone
        {
            return previous.model.clone();
        }
        let conditions = crate::incident::condition_rows(detail.data.status.as_ref());
        let model = Rc::new(Model::new(
            &detail.data.summary,
            &conditions,
            detail.events.events(),
            timezone,
        ));
        // A pending read must not evict the completed projection on reopening.
        if !matches!(detail.events.load, EventsLoad::Pending) {
            *current = Some(Current {
                data: detail.timeline_source.clone(),
                events: detail.events.layout_source.clone(),
                timezone,
                model: model.clone(),
            });
        }
        model
    }
}
