//! Brings a selection made outside the sidebar tree into view, as the web's
//! `revealKey` does. The selected row scrolls, or the collapsed branch that
//! hides it when the branch is closed.
use super::sidebar_tree::{GROUPS, TreeState};
use gpui_kit::{canvas, prelude::*, px};

impl TreeState {
    /// Selects a row chosen outside the tree and requests that it scroll into view.
    /// A kind inside a collapsed branch stays selected there. Its closest
    /// collapsed branch shows the selection, as the web does.
    pub(super) fn select_external(&mut self, path: String) {
        *self.reveal.borrow_mut() = Some(path.clone());
        self.selected = Some(path);
    }

    /// Cancels a reveal that a click on the tree has made unnecessary.
    pub(super) fn cancel_reveal(&self) {
        self.reveal.borrow_mut().take();
    }

    /// Whether this row must scroll into view: the selected row, or the closest
    /// collapsed branch that hides it, as the web's `revealKey` effect checks.
    pub(super) fn reveals(&self, path: &str, expanded: bool) -> bool {
        self.reveal.borrow().as_deref().is_some_and(|target| {
            target == path || (!expanded && target.starts_with(&format!("{path}/")))
        })
    }

    /// Measures the row this probe covers while painting and, once, scrolls the
    /// tree by the least amount that brings the row into view. Rows of a branch
    /// that just opened have their bounds on this paint. The next frame lays
    /// out the content at the new offset. An already visible row leaves the
    /// scroll position unchanged.
    pub(super) fn reveal_probe(&self) -> impl IntoElement {
        let pending = self.reveal.clone();
        let scroll = self.scroll.clone();
        canvas(
            |_, _, _| (),
            move |row, _, window, _| {
                if pending.borrow_mut().take().is_none() {
                    return;
                }
                let viewport = scroll.bounds();
                let shift = if row.top() < viewport.top() {
                    viewport.top() - row.top()
                } else if row.bottom() > viewport.bottom() {
                    viewport.bottom() - row.bottom()
                } else {
                    return;
                };
                let max = scroll.max_offset();
                let mut offset = scroll.offset();
                offset.y = (offset.y + shift).clamp(-max.y, px(0.));
                scroll.set_offset(offset);
                window.refresh();
            },
        )
        .absolute()
        .size_full()
    }
}

/// The sidebar path of a built-in kind. Cluster-scoped kinds sit under Cluster
/// Overview. Other kinds take the first group that lists them, excluding RBAC.
pub(super) fn tree_path(kind: &str) -> Option<String> {
    if crate::backend::CLUSTER_KINDS.contains(&kind) {
        return Some("Cluster Overview".to_owned());
    }
    GROUPS
        .iter()
        .filter(|(group, _)| *group != "RBAC")
        .find(|(_, kinds)| kinds.contains(&kind))
        .map(|(group, _)| format!("{group}/{kind}"))
}
