//! Focus registration follows immutable graph views.
use super::viewport;
use crate::{inspector::Inspector, topology::FlowItem};
use gpui_kit::{Context, FocusHandle, Window};
use std::{
    collections::HashMap,
    rc::{Rc, Weak},
};

impl super::MapState {
    pub(super) fn node_focus(&self, item: &FlowItem) -> Option<&FocusHandle> {
        match item {
            FlowItem::Resource(index) => self.resource_focus[*index].as_ref(),
            FlowItem::Group { .. } => self.group_focus.get(&viewport::node_id(item, self)),
        }
    }
}

impl Inspector {
    /// Keeps one focus handle per graph node and one for "skip graph". Called before
    /// the page renders, so handles follow the cached view without rebuilding it.
    ///
    /// Node and zoom handles are tab stops on the handle itself: an element's
    /// `tab_stop(true)` applies only to a handle the element creates, not to one it tracks.
    pub(in crate::inspector) fn sync_map_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _profile = crate::frame_profile::scope("map", "sync_focus");
        let unchanged = self.map.cache.as_ref().is_some_and(|view| {
            self.map
                .focus_view
                .upgrade()
                .is_some_and(|previous| Rc::ptr_eq(&previous, view))
        });
        if !unchanged {
            let mut previous = std::mem::take(&mut self.map.resource_focus);
            let mut previous_groups = std::mem::take(&mut self.map.group_focus);
            let mut resources: Vec<_> = (0..self.map.node_ids.len()).map(|_| None).collect();
            let mut groups = HashMap::new();
            if let Some(view) = &self.map.cache {
                for node in &view.nodes {
                    match &node.item {
                        FlowItem::Resource(index) => {
                            resources[*index] = Some(
                                previous[*index]
                                    .take()
                                    .unwrap_or_else(|| cx.focus_handle().tab_stop(true)),
                            );
                        }
                        FlowItem::Group { .. } => {
                            let key = viewport::node_id(&node.item, &self.map);
                            let handle = previous_groups
                                .remove(&key)
                                .unwrap_or_else(|| cx.focus_handle().tab_stop(true));
                            groups.insert(key, handle);
                        }
                    }
                }
            }
            self.map.resource_focus = resources;
            self.map.group_focus = groups;
            if self.map.zoom_focus.is_none() {
                self.map.zoom_focus =
                    Some(std::array::from_fn(|_| cx.focus_handle().tab_stop(true)));
            }
            if self.map.skip_target.is_none() {
                self.map.skip_target = Some(cx.focus_handle());
            }
            self.map.focus_view = self
                .map
                .cache
                .as_ref()
                .map_or_else(Weak::new, Rc::downgrade);
        }
        self.pan_to_newly_focused_node(window, cx);
    }

    /// Focus moving onto a node pans it into view, as xyflow's `autoPanOnNodeFocus`
    /// does. This reads focus during render rather than through `on_focus_in`,
    /// because GPUI sends focus events only while the window is active, and a focus
    /// change re-renders the window, so this runs in the frame that shows it.
    /// Only a focus change scans the node handles.
    fn pan_to_newly_focused_node(&mut self, window: &Window, cx: &Context<Self>) {
        let now = window.focused(cx);
        let same = match (&now, &self.map.last_focus) {
            (Some(now), Some(last)) => now == last,
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        let node = now.as_ref().and_then(|now| {
            self.map
                .resource_focus
                .iter()
                .position(|handle| handle.as_ref() == Some(now))
                .map(|index| self.map.node_ids[index].clone())
                .or_else(|| {
                    self.map
                        .group_focus
                        .iter()
                        .find(|(_, handle)| *handle == now)
                        .map(|(key, _)| key.clone())
                })
        });
        if let Some(key) = node {
            self.pan_map_node_into_view(&key);
        }
        self.map.last_focus = now.as_ref().map(FocusHandle::downgrade);
    }
}
