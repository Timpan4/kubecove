//! Focus registration follows immutable graph views.
use super::viewport;
use crate::inspector::Inspector;
use gpui_kit::{App, Context, Window};
use std::{
    collections::HashSet,
    rc::{Rc, Weak},
};

impl Inspector {
    /// Keeps one focus handle per graph node and one for "skip graph". Called before
    /// the page renders, so handles follow the cached view without rebuilding it.
    /// A new node's focus pans it into view, as xyflow's `autoPanOnNodeFocus` does.
    pub(in crate::inspector) fn sync_map_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _profile = crate::frame_profile::scope("map", "sync_focus");
        if let Some(view) = &self.map.cache
            && self
                .map
                .focus_view
                .upgrade()
                .is_some_and(|previous| Rc::ptr_eq(&previous, view))
        {
            return;
        }
        let mut keys = HashSet::new();
        if let Some(view) = &self.map.cache {
            for node in &view.nodes {
                let key = viewport::node_id(&node.item, &self.map.topology);
                keys.insert(key.clone());
                if self.map.node_focus.contains_key(&key) {
                    continue;
                }
                let handle = cx.focus_handle();
                let weak = cx.weak_entity();
                let pan_key = key.clone();
                let subscription =
                    window.on_focus_in(&handle, cx, move |_window: &mut Window, cx: &mut App| {
                        let _ =
                            weak.update(cx, |this, cx| this.pan_map_node_into_view(&pan_key, cx));
                    });
                self.map.node_focus.insert(key.clone(), handle);
                self.map.focus_pan.insert(key, subscription);
            }
        }
        self.map.node_focus.retain(|key, _| keys.contains(key));
        self.map.focus_pan.retain(|key, _| keys.contains(key));
        if self.map.zoom_focus.is_none() {
            self.map.zoom_focus = Some([cx.focus_handle(), cx.focus_handle(), cx.focus_handle()]);
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
}
