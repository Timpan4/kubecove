//! The launcher's "New workspace" card from `WorkspaceLauncher.svelte`.

use super::Inspector;
use super::controls::{Variant, button};
use crate::theme::Tokens;
use crate::ui;
use crate::workspaces::{self, Discovery};
use gpui_kit::TestSupportExt;
use gpui_kit::component::input::Input;
use gpui_kit::{AnyElement, Context, Div, FontWeight, SharedString, Stateful, div, prelude::*, px};

type Observed = gpui_kit::base::ObservedElement<Stateful<Div>>;

/// Adds `name` to a checkbox selection, or removes it when present.
fn toggle(selection: &mut Vec<String>, name: &str) {
    match selection.iter().position(|candidate| candidate == name) {
        Some(index) => {
            selection.remove(index);
        }
        None => selection.push(name.to_owned()),
    }
}

impl Inspector {
    pub(super) fn workspace_form(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.launcher;
        let effective = state.effective().to_owned();
        let missing = !state
            .context_choices
            .iter()
            .any(|(name, _)| *name == effective);
        let blocked = workspaces::availability(&effective, missing, state.discovery).err();
        let contexts = state
            .context_choices
            .iter()
            .map(|(name, _)| {
                let primary = *name == effective;
                let checked = primary || state.group.contains(name);
                let name = name.clone();
                check_row(
                    SharedString::from(format!("workspace-group:{name}")),
                    &name,
                    checked,
                    primary,
                )
                .when(!primary, |row| {
                    row.on_click(cx.listener(move |this, _, _, cx| {
                        toggle(&mut this.launcher.group, &name);
                        cx.notify();
                    }))
                })
            })
            .collect::<Vec<_>>();
        let namespaces: Vec<_> = state
            .discovered
            .iter()
            .map(|name| {
                let checked = state.namespaces.contains(name);
                let name = name.clone();
                check_row(
                    SharedString::from(format!("workspace-namespace:{name}")),
                    &name,
                    checked,
                    false,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    toggle(&mut this.launcher.namespaces, &name);
                    cx.notify();
                }))
            })
            .collect();
        let namespace_notice = self.namespace_notice(cx);
        let menu = state.context_menu.then(|| {
            div()
                .absolute()
                .top(px(32.))
                .left_0()
                .right_0()
                .p(px(4.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::surface2())
                .shadow_lg()
                .children(state.context_choices.iter().map(|(name, _)| {
                    let context = name.clone();
                    div()
                        .id(SharedString::from(format!("workspace-context:{name}")))
                        .test_support()
                        .aria_label(name.clone())
                        .h(px(28.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|option| option.bg(Tokens::muted()))
                        .child(ui::xs(name.clone()))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.choose_launcher_context(context.clone(), window, cx)
                        }))
                }))
        });
        ui::card()
            .flex_none()
            .w(px(360.))
            .p(px(12.))
            .gap(px(12.))
            .when(state.contexts_load == Discovery::Loading, |form| {
                form.child(
                    ui::muted(ui::xs("Loading contexts..."))
                        .id("launcher-contexts-loading")
                        .test_support()
                        .aria_label("Loading contexts..."),
                )
            })
            .when(state.context_error.is_some(), |form| {
                form.child(
                    self.friendly_failure(
                        "launcher-context-error",
                        None,
                        state
                            .context_error
                            .as_deref()
                            .unwrap_or("Context read failed"),
                        super::friendly_error::Context {
                            operation: Some(super::friendly_error::Operation::Contexts),
                            fallback_title: Some("Failed to load contexts"),
                            ..Default::default()
                        },
                        None,
                        cx,
                    ),
                )
                .child(
                    ui::outline_button()
                        .id("retry-launcher-contexts")
                        .test_support()
                        .aria_label("Retry")
                        .child("Retry")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.retry_launcher_contexts(window, cx)
                        })),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(ui::medium(ui::sm(if state.editing.is_some() {
                        "Edit workspace"
                    } else {
                        "New workspace"
                    })))
                    .when(state.editing.is_some(), |row| {
                        row.child(
                            ui::ghost_icon("x", 24., 14.)
                                .id("workspace-cancel-edit")
                                .test_support()
                                .aria_label("Cancel edit")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_workspace_edit(window, cx)
                                })),
                        )
                    }),
            )
            .child(
                label("Name").child(
                    Input::new(&state.name)
                        .id("workspace-name")
                        .aria_label("Name")
                        .text_xs()
                        .h_7(),
                ),
            )
            .child(
                label("Context").child(
                    div()
                        .relative()
                        .child(
                            div()
                                .id("workspace-context")
                                .test_support()
                                .aria_label("Context")
                                .flex()
                                .items_center()
                                .justify_between()
                                .h(px(28.))
                                .px(px(8.))
                                .rounded(px(8.))
                                .border_1()
                                .border_color(Tokens::input())
                                .bg(Tokens::input().opacity(0.3))
                                .cursor_pointer()
                                .child(context_value(
                                    &effective,
                                    state.contexts_load == Discovery::Loading,
                                ))
                                .child(ui::icon("chevron-down", 14., Tokens::muted_foreground()))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.launcher.context_menu = !this.launcher.context_menu;
                                    cx.notify();
                                })),
                        )
                        .children(menu),
                ),
            )
            .when(state.context_choices.len() > 1, |form| {
                form.child(muted_label("Cluster group").child(check_list(
                    "workspace-groups",
                    128.,
                    contexts,
                    None,
                )))
            })
            .child(muted_label("Namespaces").child(check_list(
                "workspace-namespaces",
                208.,
                namespaces,
                namespace_notice,
            )))
            .child(
                button(Variant::Default, 32., blocked.is_some())
                    .id("workspace-create")
                    .test_support()
                    .aria_label(if state.editing.is_some() {
                        "Save workspace"
                    } else {
                        "Create workspace"
                    })
                    .w_full()
                    .child(ui::icon(
                        if state.editing.is_some() {
                            "folder-open"
                        } else {
                            "plus"
                        },
                        16.,
                        Tokens::primary_foreground(),
                    ))
                    .child(if state.editing.is_some() {
                        "Save workspace"
                    } else {
                        "Create workspace"
                    })
                    .when(blocked.is_none(), |button| {
                        button.on_click(
                            cx.listener(|this, _, window, cx| this.create_workspace(window, cx)),
                        )
                    }),
            )
            .when_some(blocked, |form, reason| {
                form.child(
                    ui::muted(ui::xs(reason))
                        .id("workspace-create-disabled-reason")
                        .test_support()
                        .aria_label(reason),
                )
            })
    }

    /// The namespace list's notice for the web's loading, failed and empty states.
    fn namespace_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = &self.launcher;
        match state.discovery {
            Discovery::Failed => Some(
                div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(8.))
                    .px(px(8.))
                    .py(px(6.))
                    .child(
                        self.friendly_failure(
                            "launcher-namespace-error",
                            None,
                            state
                                .namespace_error
                                .as_deref()
                                .unwrap_or("Namespace read failed"),
                            super::friendly_error::Context {
                                operation: Some(super::friendly_error::Operation::Resources),
                                fallback_title: Some("Failed to load namespaces"),
                                partial: true,
                                compact: true,
                                ..Default::default()
                            },
                            None,
                            cx,
                        ),
                    )
                    .child(
                        ui::outline_button()
                            .id("retry-launcher-namespaces")
                            .test_support()
                            .aria_label("Retry")
                            .child("Retry")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.discover_namespaces(window, cx)
                            })),
                    )
                    .into_any_element(),
            ),
            // The web shows the loading copy only for a selected context.
            Discovery::Loading if !state.effective().is_empty() => Some(notice(
                "launcher-namespaces-loading",
                "Loading namespaces...",
            )),
            Discovery::Loading | Discovery::Ready if state.discovered.is_empty() => {
                Some(notice("launcher-namespaces-empty", "All namespaces"))
            }
            Discovery::Loading | Discovery::Ready => None,
        }
    }
}

/// The context select's value, or its placeholder while no context is chosen.
fn context_value(effective: &str, loading: bool) -> Observed {
    if effective.is_empty() {
        let placeholder = if loading {
            "Loading contexts"
        } else {
            "Select context"
        };
        ui::muted(ui::xs(placeholder))
            .id("launcher-context-placeholder")
            .test_support()
            .aria_label(placeholder)
    } else {
        ui::xs(effective.to_owned())
            .id("launcher-context-value")
            .test_support()
            .aria_label(effective.to_owned())
    }
}

/// `Field` with a `FieldLabel`: the card's `text-xs/relaxed`, `font-medium`.
fn label(text: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(ui::medium(ui::xs(text)).line_height(px(16.5)))
}

/// `FieldSet gap-1.5` with a `FieldLegend variant="label"`.
fn muted_label(text: &'static str) -> Div {
    div().flex().flex_col().gap(px(8.)).child(
        ui::muted(ui::xs(text))
            .line_height(px(19.5))
            .font_weight(FontWeight::MEDIUM)
            .id(SharedString::from(format!("launcher-label:{text}")))
            .test_support()
            .aria_label(text),
    )
}

/// Muted text inside a `ScrollArea` list, with the list's row padding.
fn notice(id: &'static str, text: &'static str) -> AnyElement {
    ui::muted(ui::xs(text))
        .id(id)
        .test_support()
        .aria_label(text)
        .px(px(8.))
        .py(px(6.))
        .into_any_element()
}

/// `ScrollArea class="h-N rounded-md border bg-background/40"` with `p-1` rows.
fn check_list(
    id: &'static str,
    height: f32,
    rows: Vec<Observed>,
    notice: Option<AnyElement>,
) -> Observed {
    div()
        .id(id)
        .test_support()
        .h(px(height))
        .overflow_y_scroll()
        .p(px(4.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::background().opacity(0.4))
        .children(rows)
        .when_some(notice, |list, notice| list.child(notice))
}

/// `h-7 gap-2 rounded-sm px-2 text-xs` with a `Checkbox`.
pub(super) fn check_row(id: SharedString, label: &str, checked: bool, disabled: bool) -> Observed {
    div()
        .id(id)
        .test_support()
        .aria_label(label.to_owned())
        .aria_toggled(if checked {
            gpui_kit::accesskit::Toggled::True
        } else {
            gpui_kit::accesskit::Toggled::False
        })
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(8.))
        .rounded(px(6.))
        .when(!disabled, |row| {
            row.cursor_pointer().hover(|row| row.bg(Tokens::muted()))
        })
        // The disabled row's checkbox and label both take `opacity-50`.
        .when(disabled, |row| row.opacity(0.5))
        // The web's `data-checked:bg-primary` never matches Bits UI's
        // `data-state`, so a checked box keeps the input surface.
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(16.))
                .rounded(px(4.))
                .border_1()
                .border_color(Tokens::input())
                .bg(Tokens::input().opacity(0.3))
                .when(checked, |box_| {
                    box_.child(ui::icon("check", 14., Tokens::foreground()))
                }),
        )
        .child(ui::xs(label.to_owned()).truncate())
}
