//! Focus registration follows immutable graph views.
use crate::{inspector::Inspector, topology::FlowItem};
use gpui_kit::{Context, FocusHandle, Window};
use std::rc::{Rc, Weak};

/// A graph node by topology index or group kind, so finding it in a view
/// compares indices instead of formatting element ids.
pub(super) enum NodeTarget {
    Resource(usize),
    Group(String),
}

impl NodeTarget {
    pub(super) fn matches(&self, item: &FlowItem) -> bool {
        match (self, item) {
            (Self::Resource(target), FlowItem::Resource(index)) => target == index,
            (Self::Group(target), FlowItem::Group { kind, .. }) => target == kind,
            _ => false,
        }
    }
}

impl super::MapState {
    pub(super) fn node_focus(&self, item: &FlowItem) -> Option<&FocusHandle> {
        match item {
            FlowItem::Resource(index) => self.resource_focus.get(*index)?.as_ref(),
            FlowItem::Group { kind, .. } => self.group_focus.get(kind.as_str()),
        }
    }
}

impl Inspector {
    /// Keeps one focus handle per graph node and one for "skip graph". Called before
    /// the page renders, so handles follow the cached view without rebuilding it.
    ///
    /// Node and zoom handles are tab stops on the handle itself: an element's
    /// `tab_stop(true)` applies only to a handle the element creates, not to one it tracks.
    ///
    /// Node handles outlive the view that created them, because opening and
    /// closing the detail pane alternates between the selected and unselected
    /// views. A handle whose node is not in the current view is never tracked,
    /// so it registers no tab stop.
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
            let count = self.map.node_ids.len();
            self.map.resource_focus.resize_with(count, || None);
            let view = self.map.cache.clone();
            // A scope change clears the view while the topology loads; with no view, every
            // node has unmounted, so a focused node handle must not carry into the new scope.
            self.drop_focus_leaving(view.as_deref(), window, cx);
            if let Some(view) = view {
                for node in &view.nodes {
                    match &node.item {
                        FlowItem::Resource(index) => {
                            self.map.resource_focus[*index]
                                .get_or_insert_with(|| cx.focus_handle().tab_stop(true));
                        }
                        FlowItem::Group { kind, .. } => {
                            if !self.map.group_focus.contains_key(kind.as_str()) {
                                self.map
                                    .group_focus
                                    .insert(kind.clone(), cx.focus_handle().tab_stop(true));
                            }
                        }
                    }
                }
            }
            if self.map.zoom_focus.is_none() {
                self.map.zoom_focus =
                    Some(std::array::from_fn(|_| cx.focus_handle().tab_stop(true)));
            }
            if self.map.mode_focus.is_none() {
                self.map.mode_focus =
                    Some(std::array::from_fn(|_| cx.focus_handle().tab_stop(true)));
            }
            if self.map.skip_target.is_none() {
                self.map.skip_target = Some(cx.focus_handle());
                self.map.skip_link = Some(cx.focus_handle().tab_stop(true));
            }
            self.map.focus_view = self
                .map
                .cache
                .as_ref()
                .map_or_else(Weak::new, Rc::downgrade);
        }
        self.pan_to_newly_focused_node(window, cx);
    }

    /// Drops the focused node's handle when `view` leaves that node out, or there is
    /// no view, so the window loses focus as it does when the focused element unmounts.
    fn drop_focus_leaving(
        &mut self,
        view: Option<&crate::topology::View>,
        window: &Window,
        cx: &Context<Self>,
    ) {
        let Some(focused) = window.focused(cx) else {
            return;
        };
        let Some(target) = self.focused_node(&focused) else {
            return;
        };
        if view.is_some_and(|view| view.nodes.iter().any(|node| target.matches(&node.item))) {
            return;
        }
        match target {
            NodeTarget::Resource(index) => self.map.resource_focus[index] = None,
            NodeTarget::Group(kind) => {
                self.map.group_focus.remove(&kind);
            }
        }
    }

    fn focused_node(&self, focused: &FocusHandle) -> Option<NodeTarget> {
        self.map
            .resource_focus
            .iter()
            .position(|handle| handle.as_ref() == Some(focused))
            .map(NodeTarget::Resource)
            .or_else(|| {
                self.map
                    .group_focus
                    .iter()
                    .find(|(_, handle)| *handle == focused)
                    .map(|(kind, _)| NodeTarget::Group(kind.clone()))
            })
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
        if let Some(target) = now.as_ref().and_then(|now| self.focused_node(now)) {
            self.pan_map_node_into_view(&target);
        }
        self.map.last_focus = now.as_ref().map(FocusHandle::downgrade);
    }
}
