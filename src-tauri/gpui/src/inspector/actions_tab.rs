//! Actions tab of the resource detail panel, mirroring
//! `ResourceActionsTab.svelte`. Row editors collect typed deltas;
//! `actions_review` renders the server dry-run review before execution.

use super::Inspector;
use super::actions_state::ActionsState;
use super::controls::{Variant, button};
use super::detail::ResourceDetail;
use super::events::tw;
use crate::actions::{self, ActionRow, RowId};
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::base::ObservedElement;
use gpui_kit::component::input::Input;
use gpui_kit::{
    AnyElement, Context, Div, SharedString, Stateful, TestSupportExt, div, prelude::*, px,
};
use kubecove_lib::models::ResourceActionKind;

/// `rounded-full border bg-muted/30 px-2 py-1` state chip.
fn chip(text: String) -> Div {
    ui::xs(text)
        .px(px(8.))
        .py(px(4.))
        .rounded_full()
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::muted().opacity(0.3))
}

fn header(actions: &ActionsState) -> Div {
    let state = actions.state.as_ref();
    let chips = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
        .when_some(state.and_then(|state| state.replicas), |row, replicas| {
            row.child(chip(format!("Desired {replicas}")))
        })
        .when_some(state.and_then(|state| state.paused), |row, paused| {
            row.child(chip(if paused { "Paused" } else { "Running" }.into()))
        })
        .when(
            state.is_some_and(|state| !state.revisions.is_empty()),
            |row| {
                let count = state.map_or(0, |state| state.revisions.len());
                row.child(chip(format!("{count} revisions")))
            },
        )
        .when(actions.loading, |row| {
            row.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::icon("loader-circle", 14., Tokens::muted_foreground()))
                    .child(ui::muted(ui::xs("Loading action permissions"))),
            )
        });
    div()
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .pb(px(12.))
        .border_b_1()
        .border_color(Tokens::border())
        .child(chips)
        .child(
            Input::new(&actions.filter)
                .id("action-filter")
                .aria_label("Resource action filter")
                .prefix(ui::icon("search", 14., Tokens::muted_foreground()))
                .suffix(ui::muted(ui::text("/", 10.4, 14.)))
                .bg(Tokens::muted().opacity(0.2))
                .text_xs()
                .h(px(32.))
                .w(px(224.)),
        )
}

fn row_button(actions: &ActionsState, row: &ActionRow, cx: &mut Context<Inspector>) -> Div {
    let state = actions.state.as_ref();
    let action = actions::action_for_row(row, state);
    let disabled = !actions::allowed(state, action)
        || actions.loading
        || (row.id == RowId::Rollback && state.is_none_or(|s| s.revisions.is_empty()));
    let expanded = actions.expanded == Some(row.id);
    let (variant, label) = match row.id {
        RowId::Delete => (Variant::Destructive, "Delete"),
        RowId::Restart => (Variant::Outline, "Restart"),
        RowId::Rollout if state.and_then(|state| state.paused) == Some(true) => {
            (Variant::Outline, "Resume")
        }
        RowId::Rollout => (Variant::Outline, "Pause"),
        RowId::Rollback if expanded => (Variant::Outline, "Close"),
        RowId::Rollback => (Variant::Outline, "Roll back"),
        _ if expanded => (Variant::Outline, "Done"),
        _ => (Variant::Outline, "Edit"),
    };
    let id = row.id;
    let button = button(variant, 24., disabled)
        .id(SharedString::from(format!("action-{}", id.key())))
        .test_support()
        .aria_label(label)
        .child(label);
    let button = if disabled {
        button
    } else {
        button.on_click(cx.listener(move |this, _, window, cx| match id {
            RowId::Delete | RowId::Restart | RowId::Rollout => {
                this.review_action(action, window, cx)
            }
            _ => this.toggle_action_row(id, window, cx),
        }))
    };
    div().child(button)
}

fn action_row(
    actions: &ActionsState,
    row: &ActionRow,
    cx: &mut Context<Inspector>,
) -> ObservedElement<Stateful<Div>> {
    let state = actions.state.as_ref();
    let action = actions::action_for_row(row, state);
    let permission = actions::permission(state, action);
    let pending = actions
        .pending_labels
        .iter()
        .find(|(id, _)| *id == row.id)
        .map(|(_, label)| label.clone());
    let mut info = div()
        .flex_1()
        .min_w_0()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::medium(ui::sm(row.label)))
                .when(pending.is_some(), |label| {
                    label.child(
                        ui::text("Pending", 11.2, 16.)
                            .px(px(8.))
                            .py(px(2.))
                            .rounded_full()
                            .bg(tw::sky500(0.1))
                            .text_color(tw::sky300()),
                    )
                }),
        )
        .child(
            ui::muted(ui::xs(
                actions
                    .current_labels
                    .iter()
                    .find(|(id, _)| *id == row.id)
                    .map(|(_, label)| label.clone())
                    .unwrap_or_default(),
            ))
            .mt(px(4.)),
        );
    if let Some(pending) = pending.clone() {
        info = info.child(ui::xs(pending).mt(px(4.)).text_color(tw::sky300()));
    }
    if permission.is_none_or(|permission| !permission.allowed) {
        let needs = permission
            .map(|permission| permission.required_permission.clone())
            .filter(|required| !required.is_empty())
            .unwrap_or_else(|| actions::fallback_permission(action, &actions.resource().kind));
        let reason = match permission {
            Some(permission) => {
                (!permission.reason.is_empty()).then(|| format!("· {}", permission.reason))
            }
            None => Some("· Permission has not been confirmed".into()),
        };
        info = info.child(
            div()
                .mt(px(4.))
                .flex()
                .items_center()
                .gap(px(4.))
                .child(ui::icon("lock-keyhole", 14., Tokens::muted_foreground()))
                .child(ui::muted(ui::xs(format!("Needs {needs}"))))
                .children(reason.map(|reason| ui::muted(ui::xs(reason)))),
        );
    }
    let controls = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .when(pending.is_some(), |controls| {
            controls.child(
                button(Variant::Ghost, 24., false)
                    .id(SharedString::from(format!(
                        "action-revert-{}",
                        row.id.key()
                    )))
                    .test_support()
                    .child("Revert")
                    .on_click(cx.listener({
                        let id = row.id;
                        move |this, _, window, cx| {
                            if let Some(actions) = this.actions_state() {
                                if id == RowId::Replicas {
                                    actions.replicas = None;
                                }
                                super::action_drafts::revert(&mut actions.edits, id);
                                actions.new_entries.retain(|kind, _| {
                                    !matches!(
                                        (id, kind),
                                        (
                                            RowId::Environment,
                                            super::action_fields::GroupKind::Environment(_)
                                        ) | (
                                            RowId::Resources,
                                            super::action_fields::GroupKind::Quantity(_, _)
                                        )
                                    )
                                });
                            }
                            this.reset_action_editor(window, cx);
                            this.refresh_action_labels(cx);
                        }
                    })),
            )
        })
        .child(row_button(actions, row, cx));
    let editor = (row.id == RowId::Replicas
        && actions.expanded == Some(RowId::Replicas)
        && actions::allowed(state, action))
    .then(|| {
        div()
            .mt(px(12.))
            .max_w(px(224.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(ui::muted(ui::xs("Desired replicas")))
            .child(
                Input::new(&actions.replicas_input)
                    .id("action-replicas-input")
                    .aria_label("Desired replicas")
                    .disabled(actions.loading)
                    .text_sm()
                    .h(px(32.)),
            )
    });
    div()
        .id(SharedString::from(format!("action-row-{}", row.id.key())))
        .test_support()
        .aria_label(row.label)
        .py(px(12.))
        .pl(px(4.))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_start()
                .justify_between()
                .gap(px(12.))
                .child(info)
                .child(controls),
        )
        .children(editor)
        .when(
            actions.expanded == Some(row.id)
                && row.id != RowId::Replicas
                && actions::allowed(state, action),
            |pane| pane.child(super::action_editor_view::render(actions, cx)),
        )
}

fn notice(id: &'static str, text: String, success: bool) -> impl IntoElement {
    let (border, background, foreground) = if success {
        (tw::emerald500(0.3), tw::emerald500(0.05), tw::emerald300())
    } else {
        (
            Tokens::destructive().opacity(0.3),
            Tokens::destructive().opacity(0.05),
            Tokens::destructive(),
        )
    };
    ui::xs(text.clone())
        .id(id)
        .test_support()
        .aria_label(text)
        .role(gpui_kit::Role::Status)
        .px(px(12.))
        .py(px(8.))
        .border_b_1()
        .border_color(border)
        .bg(background)
        .text_color(foreground)
}

pub(super) fn actions_tab(detail: &ResourceDetail, cx: &mut Context<Inspector>) -> AnyElement {
    let Some(actions) = detail.actions.as_ref() else {
        return div().into_any_element();
    };
    let visible: Vec<&ActionRow> = actions
        .rows
        .iter()
        .filter(|row| row.matches(&actions.query))
        .collect();
    let mut groups: Vec<&str> = Vec::new();
    for row in &visible {
        if !groups.contains(&row.group) {
            groups.push(row.group);
        }
    }
    // `flex h-full min-h-0 flex-col overflow-auto`: the rows scroll inside the
    // tab, and the pending bar below them stays in view as the web's sticky one.
    let mut body = div()
        .id("actions-scroll")
        .test_support()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .child(header(actions))
        .children(actions.load_error.as_ref().map(|error| {
            notice(
                "action-load-error",
                format!(
                    "Unable to load action state and permissions. Actions remain locked. {error}"
                ),
                false,
            )
        }))
        .children(
            actions
                .result
                .clone()
                .map(|result| notice("action-result", result, true)),
        );
    if actions.rows.is_empty() {
        body = body
            .child(ui::muted(ui::sm("Actions are not available for this resource.")).py(px(32.)));
    } else if visible.is_empty() {
        let filter = actions.filter.read(cx).value().to_string();
        body = body.child(ui::muted(ui::sm(format!("No action matches “{filter}”."))).py(px(32.)));
    }
    for group in groups {
        let rows: Vec<&&ActionRow> = visible.iter().filter(|row| row.group == group).collect();
        let last = rows.len().saturating_sub(1);
        body = body
            .when(!group.is_empty(), |body| {
                body.child(
                    ui::semibold(ui::muted(ui::xs(group)))
                        .mt(px(16.))
                        .px(px(4.))
                        .pb(px(8.))
                        .border_b_1()
                        .border_color(Tokens::border()),
                )
            })
            .child(div().children(rows.iter().enumerate().map(|(index, row)| {
                action_row(actions, row, cx).when(index < last, |row| {
                    row.border_b_1().border_color(Tokens::border())
                })
            })));
    }
    let pending = (actions.pending() > 0).then(|| {
        let can_check =
            actions::allowed(actions.state.as_ref(), ResourceActionKind::Edit) && !actions.loading;
        div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .px(px(12.))
            .py(px(8.))
            .border_t_1()
            .border_color(Tokens::border())
            .bg(Tokens::background())
            .shadow_sm()
            .child(ui::muted(ui::xs(format!(
                "{} pending changes",
                actions.pending()
            ))))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        button(Variant::Ghost, 24., false)
                            .id("action-discard")
                            .test_support()
                            .child("Discard")
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(actions) = this.actions_state() {
                                    actions.replicas = None;
                                    actions.edits = Default::default();
                                }
                                this.reset_action_editor(window, cx);
                                this.refresh_action_labels(cx);
                            })),
                    )
                    .child({
                        let check = button(Variant::Default, 24., !can_check)
                            .id("action-check")
                            .test_support()
                            .child("Check changes");
                        if can_check {
                            check.on_click(cx.listener(|this, _, window, cx| {
                                this.review_action(ResourceActionKind::Edit, window, cx)
                            }))
                        } else {
                            check
                        }
                    }),
            )
    });
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(body)
        .children(pending)
        .into_any_element()
}
