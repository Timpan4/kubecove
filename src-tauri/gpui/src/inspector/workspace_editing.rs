//! Saved workspace form edits keep their stable identity and retained metadata.
use super::Inspector;
use crate::workspaces::{self, Draft};
use gpui_kit::{Context, Window};

impl Inspector {
    pub(super) fn edit_workspace(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(saved) = self
            .launcher
            .saved
            .iter()
            .find(|saved| saved.id == id)
            .cloned()
        else {
            return;
        };
        let state = &mut self.launcher;
        state.editing = Some(id);
        state.context = saved.context.clone();
        state.group = saved
            .contexts()
            .into_iter()
            .filter(|context| *context != saved.context)
            .map(str::to_owned)
            .collect();
        state.namespaces = saved.namespaces.clone();
        let name = saved.name.clone();
        state
            .name
            .update(cx, |input, cx| input.set_value(name, window, cx));
        self.discover_namespaces(window, cx);
    }

    pub(super) fn cancel_workspace_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.launcher.editing = None;
        self.launcher.context.clear();
        self.launcher.group.clear();
        self.launcher.namespaces.clear();
        self.launcher
            .name
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.discover_namespaces(window, cx);
    }

    pub(super) fn create_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.launcher;
        let effective = state.effective().to_owned();
        let missing = !state
            .context_choices
            .iter()
            .any(|(name, _)| *name == effective);
        if workspaces::availability(&effective, missing, state.discovery).is_err() {
            return;
        }
        let name = state.name.read(cx).value();
        let editing = state.editing;
        let previous = editing
            .and_then(|id| state.saved.iter().find(|saved| saved.id == id))
            .cloned();
        // A deleted edit target must never silently become a new workspace.
        if editing.is_some() && previous.is_none() {
            return;
        }
        let order = state.next;
        state.next = state.next.saturating_add(1);
        let id = editing.unwrap_or(order);
        let mut workspace = Draft {
            name: &name,
            effective: &effective,
            group: &state.group,
            namespaces: &state.namespaces,
            kinds: previous.as_ref().map(|saved| saved.kinds.as_slice()),
        }
        .build(id, order);
        if let Some(previous) = previous {
            workspace.reviews = previous.reviews;
            let contexts = workspace
                .contexts()
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            workspace
                .reviews
                .retain(|review| contexts.contains(&review.cluster_context));
            if state.active == Some(id) {
                self.rbac.reviews = workspace.reviews.clone();
            }
            if let Some(document) = state.transfer.documents.get_mut(&id) {
                document["spec"]["shortcuts"] = serde_json::to_value(
                    crate::workspaces::shortcuts::generated(&workspace, Some(document)),
                )
                .expect("shortcut identities serialize");
            }
            if let Some(saved) = state.saved.iter_mut().find(|saved| saved.id == id) {
                *saved = workspace;
            }
            self.save_preferences(cx);
            self.cancel_workspace_edit(window, cx);
        } else {
            state.saved.push(workspace);
            self.save_preferences(cx);
            self.open_workspace(id, window, cx);
        }
    }
}
