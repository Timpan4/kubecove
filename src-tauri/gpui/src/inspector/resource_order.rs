//! Focused Arrow keys match ResourceKindOrderSettings.svelte.
use super::Inspector;
use gpui_kit::Context;

#[derive(Clone)]
pub(super) struct DragKind(pub String);

impl gpui_kit::Render for DragKind {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        _: &mut Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        crate::ui::sm(self.0.clone())
    }
}

impl Inspector {
    /// Records the kind being dragged. Notifies only when it changes.
    pub(super) fn track_kind_drag(&mut self, kind: String, cx: &mut Context<Self>) {
        if self.settings.kind_drag.as_deref() != Some(kind.as_str()) {
            self.settings.kind_drag = Some(kind);
            cx.notify();
        }
    }
    /// Clears the drag highlight when the pointer is released.
    pub(super) fn end_kind_drag(&mut self, cx: &mut Context<Self>) {
        if self.settings.kind_drag.take().is_some() {
            cx.notify();
        }
    }
    pub(super) fn drop_kind(
        &mut self,
        kind: &str,
        lane: &str,
        before: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let lanes = &mut self.persisted.preferences.resource_kind_lanes;
        let all = [&mut lanes.first, &mut lanes.normal, &mut lanes.last];
        if !all
            .iter()
            .any(|items| items.iter().any(|item| item == kind))
        {
            return;
        }
        let destination = match lane {
            "Show first" => 0,
            "Normal" => 1,
            "Show last" => 2,
            _ => return,
        };
        if before == Some(kind) {
            return;
        }
        if before.is_none() && all[destination].iter().any(|item| item == kind) {
            return;
        }
        let index = before
            .and_then(|before| all[destination].iter().position(|item| item == before))
            .unwrap_or(all[destination].len());
        for items in all {
            items.retain(|item| item != kind);
        }
        let target = match destination {
            0 => &mut lanes.first,
            1 => &mut lanes.normal,
            _ => &mut lanes.last,
        };
        target.insert(index.min(target.len()), kind.to_owned());
        self.save_preferences(cx);
    }
    pub(super) fn move_kind_key(&mut self, kind: &str, key: &str, cx: &mut Context<Self>) -> bool {
        let lanes = &mut self.persisted.preferences.resource_kind_lanes;
        let all = [&mut lanes.first, &mut lanes.normal, &mut lanes.last];
        let Some(lane) = all
            .iter()
            .position(|items| items.iter().any(|item| item == kind))
        else {
            return false;
        };
        let index = all[lane]
            .iter()
            .position(|item| item == kind)
            .expect("located kind");
        let (destination, position) = match key {
            "up" if index > 0 => (lane, index - 1),
            "down" if index + 1 < all[lane].len() => (lane, index + 1),
            "left" if lane > 0 => (lane - 1, index),
            "right" if lane + 1 < all.len() => (lane + 1, index),
            _ => return false,
        };
        let kind = all[lane].remove(index);
        let position = position.min(all[destination].len());
        all[destination].insert(position, kind);
        self.save_preferences(cx);
        true
    }
}
