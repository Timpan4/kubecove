//! Workspace launcher, mirroring `WorkspaceLauncher.svelte` in the app's
//! launcher shell. Local saved records persist through Backend under ADR 0030.
//! Opening a workspace sets the context and limits rows and the sidebar to its namespaces.

use super::Inspector;
use super::controls::{Variant, button};
use crate::theme::{Palette, Tokens};
use crate::ui;
use crate::workspaces::{self, Discovery, SavedWorkspace};
use gpui_kit::TestSupportExt;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    AnyElement, Context, Div, Entity, SharedString, Stateful, Task, Window, div, prelude::*, px,
};
use kubecove_lib::models::ResourceSummary;

pub(super) struct LauncherState {
    pub(super) transfer: super::launcher_transfer::TransferState,
    pub(super) open: bool,
    pub(super) saved: Vec<SavedWorkspace>,
    /// The open workspace. `None` leaves only the launcher and settings, as the web's shell is
    /// absent without a selected workspace.
    pub(super) active: Option<u64>,
    /// Ids and edit order for new records.
    pub(super) next: u64,
    pub(super) name: Entity<InputState>,
    /// Kube contexts by name, with whether each is current.
    pub(super) context_choices: Vec<(String, bool)>,
    /// The form's context; empty means the current one.
    pub(super) context: String,
    pub(super) editing: Option<u64>,
    pub(super) context_menu: bool,
    pub(super) group: Vec<String>,
    pub(super) namespaces: Vec<String>,
    pub(super) discovered: Vec<String>,
    pub(super) discovery: Discovery,
    pub(super) contexts_load: Discovery,
    pub(super) context_error: Option<String>,
    pub(super) namespace_error: Option<String>,
    /// Kubeconfig source and context reads kept while fresh, as the web's query cache keeps them.
    pub(super) kubeconfig: super::kubeconfig_reads::KubeconfigReads,
    tasks: [Option<Task<()>>; 2],
}

impl LauncherState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        Self {
            transfer: Default::default(),
            // Startup has no open workspace, so the launcher is the first view, as in the web.
            open: true,
            saved: vec![],
            active: None,
            next: 1,
            name: cx.new(|cx| InputState::new(window, cx)),
            context_choices: vec![],
            context: String::new(),
            editing: None,
            context_menu: false,
            group: vec![],
            namespaces: vec![],
            discovered: vec![],
            discovery: Discovery::Loading,
            contexts_load: Discovery::Loading,
            context_error: None,
            namespace_error: None,
            kubeconfig: Default::default(),
            tasks: [None, None],
        }
    }

    fn active_namespaces(&self) -> &[String] {
        self.active
            .and_then(|id| self.saved.iter().find(|saved| saved.id == id))
            .map_or(&[], |saved| saved.namespaces.as_slice())
    }

    pub(super) fn overview_kind(&self, kind: &str) -> bool {
        self.active
            .and_then(|id| self.saved.iter().find(|saved| saved.id == id))
            .map_or_else(
                || crate::model::WORKSPACE_KINDS.contains(&kind),
                |saved| saved.kinds.iter().any(|saved| saved == kind),
            )
    }

    /// Rows in the open workspace's namespaces; cluster-scoped rows always.
    pub(super) fn in_scope(&self, row: &ResourceSummary) -> bool {
        let namespaces = self.active_namespaces();
        namespaces.is_empty()
            || row
                .namespace
                .as_ref()
                .is_none_or(|namespace| namespaces.contains(namespace))
    }

    pub(super) fn shows_namespace(&self, namespace: &str) -> bool {
        let namespaces = self.active_namespaces();
        namespaces.is_empty() || namespaces.iter().any(|candidate| candidate == namespace)
    }

    pub(super) fn effective(&self) -> &str {
        workspaces::effective_context(&self.context, &self.context_choices)
    }
}

/// The web's "No saved workspaces yet" empty state.
fn no_saved_workspaces() -> gpui_kit::base::ObservedElement<Stateful<Div>> {
    div()
        .id("workspaces-empty")
        .test_support()
        .aria_label("No saved workspaces yet")
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .min_h(px(208.))
        .p(px(24.))
        .rounded(px(10.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .child(ui::icon("plus", 20., Tokens::muted_foreground()))
        .child(ui::medium(ui::sm("No saved workspaces yet")))
        .child(
            ui::muted(ui::xs(
                "Create a workspace from a cluster context to start inspecting resources, topology, and live sessions.",
            ))
            .text_center(),
        )
}

/// The web's "Context unavailable" badge for a saved workspace whose context is missing.
fn context_unavailable(name: &str) -> gpui_kit::base::ObservedElement<Stateful<Div>> {
    div()
        .id(SharedString::from(format!("workspace-unavailable:{name}")))
        .test_support()
        .aria_label(format!("{name} Context unavailable"))
        .flex()
        .items_center()
        .h(px(20.))
        .px(px(6.))
        .rounded(px(4.))
        .border_1()
        .border_color(Palette::amber500(0.4))
        .child(ui::xs("Context unavailable").text_color(Palette::amber300()))
}

impl Inspector {
    pub(super) fn open_launcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Opening the launcher closes the workspace, which would drop an unsaved draft.
        if self.workspace_open() && !self.can_leave(cx) {
            return;
        }
        if self.palette.open {
            self.close_palette(window, cx);
        }
        // The web clears the selected workspace when the launcher opens. Its shell, detail and
        // live sessions go with it. The record keeps the view state that its next open restores.
        if self.workspace_open() {
            self.stop_sessions_outside_workspace(cx);
            self.remember_workspace(cx);
            self.store_active_reviews();
            self.close_detail(cx);
            self.launcher.active = None;
        }
        let state = &mut self.launcher;
        state.open = true;
        state.editing = None;
        state.context.clear();
        state.context_menu = false;
        state.group.clear();
        state.namespaces.clear();
        state
            .name
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.load_launcher_contexts(false, window, cx);
    }

    /// Loads the launcher's contexts. `force` is Retry, which refetches the source and contexts.
    pub(super) fn load_launcher_contexts(
        &mut self,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.launcher.contexts_load = Discovery::Loading;
        self.launcher.context_error = None;
        self.launcher.tasks[1] = None;
        let task = self.read_kubeconfig_contexts(
            force,
            window,
            cx,
            |this, result, source_error, window, cx| {
                match result {
                    Ok(contexts) => {
                        this.launcher.context_choices = contexts
                            .into_iter()
                            .map(|context| (context.name, context.is_current))
                            .collect();
                        this.launcher.contexts_load = Discovery::Ready;
                        this.launcher.context_error = source_error;
                        this.discover_namespaces(window, cx);
                        this.update_saved_overview();
                    }
                    Err(error) => {
                        this.launcher.contexts_load = Discovery::Failed;
                        this.launcher.context_error = Some(source_error.unwrap_or(error.message));
                    }
                }
                cx.notify();
            },
        );
        self.launcher.tasks[0] = Some(task);
        cx.notify();
    }

    /// Namespace discovery for the form's effective context.
    pub(super) fn discover_namespaces(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.launcher;
        let effective = state.effective().to_owned();
        state.name.update(cx, |input, cx| {
            input.set_placeholder(effective.clone(), window, cx)
        });
        state.discovery = Discovery::Loading;
        state.namespace_error = None;
        state.discovered.clear();
        if effective.is_empty() {
            state.tasks[1] = None;
            state.discovery = Discovery::Ready;
            cx.notify();
            return;
        }
        let read = crate::backend::namespaces_for_context(self.backend.clone(), effective);
        let read = self.timings.measure("namespaces", read);
        state.tasks[1] = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.launcher;
                match result {
                    Ok(namespaces) => {
                        state.discovered = namespaces.into_iter().map(|ns| ns.name).collect();
                        state.discovery = Discovery::Ready;
                    }
                    Err(error) => {
                        state.discovery = Discovery::Failed;
                        state.namespace_error = Some(error.message);
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn choose_launcher_context(
        &mut self,
        context: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.launcher;
        state.context_menu = false;
        if state.effective() != context {
            state.context = context;
            state.namespaces.clear();
            state.group.retain(|member| member != &state.context);
            self.discover_namespaces(window, cx);
        }
        cx.notify();
    }

    pub(super) fn open_workspace(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.defer_exec_leave(super::exec_leave::LeaveIntent::Workspace(id), cx) {
            return;
        }
        if !self.can_leave(cx) {
            return;
        }
        self.remember_workspace(cx);
        self.store_active_reviews();
        let Some(workspace) = self.launcher.saved.iter().find(|saved| saved.id == id) else {
            return;
        };
        self.rbac.reviews = workspace.reviews.clone();
        self.rbac.review_editor_key = None;
        self.palette.kind = None;
        self.workspace_name = workspace.name.clone().into();
        let context = workspace.context.clone();
        self.launcher.active = Some(id);
        self.launcher.open = false;
        self.close_detail(cx);
        self.restore_workspace_memory(id, &context, window, cx);
        self.context_picker.update(cx, |picker, cx| {
            picker.set_selected_value(&SharedString::from(context), window, cx)
        });
        self.load_scope(false, window, cx);
        if self.persisted.preferences.auto_start_saved_port_forwards {
            self.restore_forwards(window, cx);
        }
    }

    pub(super) fn sync_workspace_reviews(&mut self) {
        if let Some(id) = self.launcher.active
            && let Some(workspace) = self.launcher.saved.iter_mut().find(|s| s.id == id)
        {
            workspace.reviews = self.rbac.reviews.clone();
        }
    }

    /// `deleteWorkspace` in the web's workspace store: removes the record, and clears the open
    /// workspace when the deleted record is that one. The launcher stays open with no workspace.
    /// Deleting the open workspace also stops its live sessions, as the web's cleanup does when the
    /// selected workspace clears.
    fn delete_workspace(&mut self, id: u64, cx: &mut Context<Self>) {
        let state = &mut self.launcher;
        state.saved.retain(|saved| saved.id != id);
        let deleted_open = state.active == Some(id);
        if deleted_open {
            state.active = None;
            state.open = true;
        }
        self.navigation.forget(id);
        self.save_preferences(cx);
        if deleted_open {
            self.stop_sessions_outside_workspace(cx);
        }
        cx.notify();
    }

    /// Whether a workspace is open. The shell, its sidebar, palette and header controls, and every
    /// workspace page exist only then. Without one, only the launcher and settings render.
    pub(super) fn workspace_open(&self) -> bool {
        self.launcher.active.is_some()
    }

    /// Copies the open workspace's RBAC reviews into its record before the record is replaced or
    /// closed.
    fn store_active_reviews(&mut self) {
        if let Some(active) = self.launcher.active
            && let Some(workspace) = self
                .launcher
                .saved
                .iter_mut()
                .find(|saved| saved.id == active)
        {
            workspace.reviews = std::mem::take(&mut self.rbac.reviews);
        }
    }

    /// The launcher and command palette, drawn over the workspace.
    pub(super) fn overlays(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut overlays = Vec::new();
        if let Some(dialog) = self.argo_sync_dialog(cx) {
            overlays.push(dialog);
        }
        if let Some(dialog) = self.argo_connection_dialog(cx) {
            overlays.push(dialog);
        }
        if let Some(dialog) = self.exec_leave_dialog(cx) {
            overlays.push(dialog);
        }
        if self.launcher.open {
            overlays.push(self.launcher_view(window, cx).into_any_element());
        }
        if self.palette.open {
            overlays.push(self.palette_overlay(cx));
        }
        overlays
    }

    fn launcher_view(&self, _: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("launcher")
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .bg(Tokens::background())
            .font_family(crate::theme::SANS)
            .text_color(Tokens::foreground())
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation()
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .h(px(50.))
                    .px(px(16.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::card())
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .child(ui::semibold(ui::sm("KubeCove")))
                            .child(ui::muted(ui::xs("Workspaces"))),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .child(ui::medium(ui::sm("Workspaces"))),
                    )
                    .child(
                        ui::ghost_icon("settings", 28., 14.)
                            .id("launcher-settings")
                            .test_support()
                            .aria_label("Open settings")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.launcher.open = false;
                                this.open_settings(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("launcher-body")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .justify_center()
                    .overflow_y_scroll()
                    .p(px(24.))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(16.))
                            .w_full()
                            .max_w(px(1152.))
                            .mx_auto()
                            .child(self.saved_workspaces(cx))
                            .child(self.workspace_form(cx)),
                    ),
            )
    }

    fn saved_workspaces(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.launcher;
        let mut saved: Vec<&SavedWorkspace> = state.saved.iter().collect();
        saved.sort_by_key(|workspace| std::cmp::Reverse(workspace.updated));
        let empty = saved.is_empty() && self.launcher_contexts_loading().ready();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .mb(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .child(
                                ui::semibold(ui::text("Workspaces", 16., 24.))
                                    .font_family(crate::theme::HEADING),
                            )
                            .child(ui::muted(ui::xs(format!("{} saved", saved.len())))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                button(Variant::Outline, 24., state.transfer.busy)
                                    .id("settings-button:Import")
                                    .test_support()
                                    .aria_label("Import")
                                    .child(ui::icon("upload", 12., Tokens::foreground()))
                                    .child("Import")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.import_workspace_file(window, cx)
                                    })),
                            )
                            .child(
                                button(Variant::Outline, 24., state.transfer.busy)
                                    .id("settings-button:Export all")
                                    .test_support()
                                    .aria_label("Export all")
                                    .child(ui::icon("download", 12., Tokens::foreground()))
                                    .child("Export all")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.export_workspace_file(None, window, cx)
                                    })),
                            ),
                    ),
            )
            .child(self.workspace_transfer_panel(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .when(empty, |list| list.child(no_saved_workspaces()))
                    .children(saved.into_iter().map(|workspace| {
                        let id = workspace.id;
                        let unavailable = !state.context_choices.is_empty()
                            && !state
                                .context_choices
                                .iter()
                                .any(|(name, _)| name == &workspace.context);
                        div()
                            .id(("workspace", id as usize))
                            .test_support()
                            .aria_label(workspace.name.clone())
                            .aria_description(workspace.summary())
                            .flex()
                            .items_start()
                            .gap(px(8.))
                            .pt(px(12.))
                            .pb(px(24.))
                            .rounded(px(10.))
                            .border_1()
                            .border_color(if unavailable {
                                Palette::amber500(0.4)
                            } else {
                                Tokens::border()
                            })
                            .bg(Tokens::card())
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(4.))
                                    .px(px(12.))
                                    .child(ui::medium(ui::sm(workspace.name.clone())).truncate())
                                    .when(unavailable, |column| {
                                        column.child(context_unavailable(&workspace.name))
                                    })
                                    .child(
                                        ui::muted(ui::xs(workspace.summary()))
                                            .line_height(px(19.5))
                                            .truncate(),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .px(px(12.))
                                    .child(
                                        button(Variant::Default, 24., false)
                                            .id(("workspace-open", id as usize))
                                            .test_support()
                                            .aria_label(format!("Open {}", workspace.name))
                                            .child(ui::icon(
                                                "folder-open",
                                                12.,
                                                Tokens::primary_foreground(),
                                            ))
                                            .child("Open")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.open_workspace(id, window, cx)
                                            })),
                                    )
                                    .child(
                                        ui::ghost_icon("download", 24., 14.)
                                            .id(("workspace-export", id as usize))
                                            .test_support()
                                            .aria_label(format!("Export {}", workspace.name))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.export_workspace_file(Some(id), window, cx)
                                            })),
                                    )
                                    .child(
                                        ui::ghost_icon("pencil", 24., 14.)
                                            .id(("workspace-edit", id as usize))
                                            .test_support()
                                            .aria_label(format!("Edit {}", workspace.name))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.edit_workspace(id, window, cx)
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id(("workspace-delete", id as usize))
                                            .test_support()
                                            .aria_label(format!("Delete {}", workspace.name))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.delete_workspace(id, cx)
                                            }))
                                            .child(
                                                ui::ghost_icon("trash-2", 24., 14.)
                                                    .text_color(Tokens::muted_foreground()),
                                            ),
                                    ),
                            )
                    })),
            )
    }
}
