//! Rendering for the cached Actions row editors in ResourceActionsTab.svelte.
use super::controls::{Variant, button};
use super::{Inspector, action_fields::*, actions_state::ActionsState};
use crate::{actions::RowId, theme::Tokens, ui};
use gpui_kit::component::input::Input;
use gpui_kit::{Context, Div, TestSupportExt, div, prelude::*, px};

pub(super) fn render(a: &ActionsState, cx: &mut Context<Inspector>) -> Div {
    let mut body = div()
        .mt(px(12.))
        .pl(px(12.))
        .border_l_1()
        .border_color(Tokens::border())
        .flex()
        .flex_col()
        .gap(px(if a.expanded == Some(RowId::Resources) {
            12.
        } else {
            16.
        }));
    if a.expanded == Some(RowId::Rollback) {
        return super::action_rollback_view::render(a, cx);
    }
    for (gi, g) in a.editor.groups.iter().enumerate() {
        let mut section = div().flex().flex_col().gap(px(8.)).when(
            !g.title.is_empty() && !matches!(g.kind, GroupKind::Quantity(_, _)),
            |d| d.child(ui::semibold(ui::xs(g.title.clone()))),
        );
        if let GroupKind::Quantity(_, limits) = &g.kind {
            if !limits {
                section = section.child(ui::semibold(ui::xs(g.title.clone())));
            }
            section = section.child(ui::medium(ui::xs(if *limits {
                "Limits"
            } else {
                "Requests"
            })));
        }
        if matches!(g.kind, GroupKind::Image(_)) {
            for ei in 0..g.entries.len() {
                section = section.child(render_entry(a, gi, ei, cx));
            }
        } else if !g.entries.is_empty() {
            // Rows use the web's h-7 inputs and gap-2. The viewport, rather
            // than a fixed entry cap, determines which rows GPUI mounts.
            section = section.child(
                gpui_kit::uniform_list(
                    g.entries_id.clone(),
                    g.entries.len(),
                    cx.processor(
                        move |this: &mut Inspector, range: std::ops::Range<usize>, _, cx| {
                            let super::detail::Detail::Ready(d) = &this.detail else {
                                return Vec::new();
                            };
                            let Some(a) = &d.actions else {
                                return Vec::new();
                            };
                            range
                                .map(|ei| {
                                    gpui_kit::div()
                                        .h(px(36.))
                                        .child(render_entry(a, gi, ei, cx))
                                })
                                .collect()
                        },
                    ),
                )
                .h(px(g.entries.len() as f32 * 36. - 8.))
                .max_h(a.editor.viewport),
            );
        }
        if !matches!(g.kind, GroupKind::Image(_)) {
            let (name, value) = (g.name_label.clone(), g.value_label.clone());
            section = section.child(
                div()
                    .id(g.new_controls_label.clone())
                    .test_support()
                    .aria_label(g.new_controls_label.clone())
                    .flex()
                    .gap(px(8.))
                    .child(
                        Input::new(&g.name)
                            .id(name.clone())
                            .aria_label(name)
                            .text_size(px(12.))
                            .line_height(px(19.5))
                            .h(px(28.))
                            .w(px(128.)),
                    )
                    .child(
                        div().flex_1().min_w(px(160.)).child(
                            Input::new(&g.value)
                                .id(value.clone())
                                .aria_label(value)
                                .text_size(px(12.))
                                .line_height(px(19.5))
                                .h(px(28.)),
                        ),
                    )
                    .child(
                        button(
                            Variant::Outline,
                            24.,
                            g.name.read(cx).value().trim().is_empty(),
                        )
                        .id(g.add_id.clone())
                        .test_support()
                        .aria_label("Add")
                        .child("Add")
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.add_action_entry(gi, window, cx),
                        )),
                    ),
            );
        }
        body = body.child(section);
    }
    body
}

fn render_entry(
    a: &ActionsState,
    gi: usize,
    ei: usize,
    cx: &mut Context<Inspector>,
) -> gpui_kit::AnyElement {
    let g = &a.editor.groups[gi];
    let e = &g.entries[ei];
    let key_label = e.key_label.clone();
    let value_label = e.value_label.clone();
    let image = matches!(g.kind, GroupKind::Image(_));
    let new_env = matches!(&g.kind, GroupKind::Environment(name) if a.state.as_ref().is_some_and(|base| base.containers.iter().find(|c| &c.name == name).is_some_and(|c| !c.env.iter().any(|v| v.name == e.key))));
    div()
        .id(e.controls_label.clone())
        .test_support()
        .aria_label(e.controls_label.clone())
        .flex()
        .items_center()
        .gap(px(8.))
        .when(!image, |d| {
            d.child(
                Input::new(&e.key_input)
                    .id(key_label.clone())
                    .aria_label(key_label)
                    .disabled(!new_env)
                    .text_size(px(12.))
                    .line_height(px(19.5))
                    .h(px(28.))
                    .w(px(128.)),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w(px(160.))
                .when(image, |d| d.child(ui::muted(ui::xs("Container image"))))
                .child(
                    Input::new(&e.input)
                        .id(e.value_id.clone())
                        .aria_label(value_label)
                        .disabled(
                            (e.removed && matches!(g.kind, GroupKind::Environment(_))) || a.loading,
                        )
                        .text_size(px(12.))
                        .line_height(px(19.5))
                        .h(px(28.)),
                ),
        )
        .when(!image, |d| {
            d.child(
                button(Variant::Ghost, 24., a.loading)
                    .id(e.remove_id.clone())
                    .test_support()
                    .aria_label(if e.removed { "Restore" } else { "Remove" })
                    .child(if e.removed { "Restore" } else { "Remove" })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.remove_action_entry(gi, ei, window, cx)
                    })),
            )
        })
        .into_any_element()
}
