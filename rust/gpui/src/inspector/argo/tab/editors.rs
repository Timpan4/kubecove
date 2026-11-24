//! The read-only YAML editors of a snapshot's changes. A document gets its
//! editor the first time its article shows it, as `Body::of` decides.
use super::ArgoLoad;
use crate::inspector::argo::snapshot::Body;
use crate::inspector::{Inspector, yaml};
use gpui_kit::component::input::EditorState;
use gpui_kit::{Context, Entity, Window, prelude::*};

/// A change's desired and live editors, each built once its document shows.
#[derive(Default)]
pub(in crate::inspector::argo) struct Editors {
    desired: Option<Entity<EditorState>>,
    live: Option<Entity<EditorState>>,
}

impl Editors {
    /// The live document's editor, or else the desired one's, once built.
    pub(in crate::inspector::argo) fn get(&self, live: bool) -> Option<&Entity<EditorState>> {
        if live {
            self.live.as_ref()
        } else {
            self.desired.as_ref()
        }
    }
}

impl Inspector {
    /// Builds the editor of each YAML document the snapshot shows, the first
    /// time it shows. A collapsed removal, a diff, a note and the document a
    /// view does not show get none.
    pub(in crate::inspector) fn ensure_argo_editors(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.argo_tab_state() else {
            return;
        };
        let ArgoLoad::Ready(view) = &state.load else {
            return;
        };
        let count = view.changes.len();
        state.editors.resize_with(count, Editors::default);
        let shown = match state.focus.filter(|index| *index < count) {
            Some(index) => index..index + 1,
            None => 0..count,
        };
        for index in shown {
            let change = &view.changes[index];
            let Body::Yaml { live } = Body::of(state.view, change, state.expanded.contains(&index))
            else {
                continue;
            };
            let editors = &mut state.editors[index];
            let slot = if live {
                &mut editors.live
            } else {
                &mut editors.desired
            };
            if slot.is_none() {
                let text = if live {
                    change.live.clone()
                } else {
                    change.desired.clone()
                };
                *slot = Some(cx.new(|cx| yaml::viewer(text, window, cx)));
            }
        }
    }
}
