//! Namespaces and discovered kinds of a saved workspace's primary context.
//! The web reads both with their own queries, so restore warnings neither wait
//! for the resource load nor check another context's inventory.
use super::Inspector;
use gpui_kit::{Context, Task};
use std::rc::Rc;

#[derive(Default)]
pub(super) struct RestoreInventory {
    pub(super) context: String,
    /// `None` until the read settles. A failed read counts as an empty
    /// inventory, as the web's `data ?? []` does.
    pub(super) namespaces: Option<Vec<String>>,
    pub(super) kinds: Option<Vec<String>>,
    _tasks: Vec<Task<()>>,
}

impl Inspector {
    /// The primary context of the active saved workspace.
    pub(super) fn active_saved_context(&self) -> Option<String> {
        self.launcher
            .active
            .and_then(|id| self.launcher.saved.iter().find(|saved| saved.id == id))
            .map(|saved| saved.context.clone())
    }

    /// Starts the namespace and kind reads for the active workspace's primary context.
    pub(super) fn load_restore_inventory(&mut self, cx: &mut Context<Self>) {
        let Some(context) = self.active_saved_context() else {
            self.restore_inventory = None;
            return;
        };
        let namespaces =
            crate::backend::namespaces_for_context(self.backend.clone(), context.clone());
        let kinds = self.backend.resource_kinds(context.clone());
        let namespace_request = self.request.clone();
        let kind_request = self.request.clone();
        let namespace_context = context.clone();
        let kind_context = context.clone();
        let namespace_task = cx.spawn(async move |this, cx| {
            let names: Vec<String> = namespaces
                .await
                .map_or_default(|items| items.into_iter().map(|item| item.name).collect());
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &namespace_request) {
                    return;
                }
                if let Some(inventory) = this
                    .restore_inventory
                    .as_mut()
                    .filter(|inventory| inventory.context == namespace_context)
                {
                    inventory.namespaces = Some(names);
                }
                this.update_saved_overview();
                cx.notify();
            });
        });
        let kind_task = cx.spawn(async move |this, cx| {
            let kinds: Vec<String> = kinds
                .await
                .map_or_default(|items| items.into_iter().map(|item| item.kind).collect());
            let _ = this.update(cx, |this, cx| {
                if !Rc::ptr_eq(&this.request, &kind_request) {
                    return;
                }
                if let Some(inventory) = this
                    .restore_inventory
                    .as_mut()
                    .filter(|inventory| inventory.context == kind_context)
                {
                    inventory.kinds = Some(kinds);
                }
                this.update_saved_overview();
                cx.notify();
            });
        });
        self.restore_inventory = Some(RestoreInventory {
            context,
            namespaces: None,
            kinds: None,
            _tasks: vec![namespace_task, kind_task],
        });
    }
}
