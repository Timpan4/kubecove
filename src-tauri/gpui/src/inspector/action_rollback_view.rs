//! Revision radio rows from ResourceActionsTab.svelte: gap-2, border, px-3 py-2.
use super::{
    Inspector,
    actions_state::ActionsState,
    controls::{Variant, button},
};
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::component::{Sizable, Size, radio::Radio};
use gpui_kit::{Context, Div, SharedString, div, prelude::*, px};
use kubecove_lib::models::ResourceActionRevision;

pub(super) struct Revision {
    revision: i64,
    id: SharedString,
    radio_id: SharedString,
    label: SharedString,
    images: SharedString,
}
impl Revision {
    pub fn new(r: &ResourceActionRevision) -> Self {
        Self {
            revision: r.revision,
            id: format!("action-revision:{}", r.revision).into(),
            radio_id: format!("action-revision-radio:{}", r.revision).into(),
            label: format!("Revision {}", r.revision).into(),
            images: r.images.join(", ").into(),
        }
    }
}
fn row(a: &ActionsState, index: usize, cx: &mut Context<Inspector>) -> impl IntoElement {
    let r = &a.editor.revisions[index];
    let revision = r.revision;
    div()
        .id(r.id.clone())
        .test_support()
        .aria_label(r.label.clone())
        .aria_selected(a.revision == Some(revision))
        .h(px(34.))
        .w_full()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .rounded(px(6.))
        .cursor_pointer()
        .child(
            Radio::new(r.radio_id.clone())
                .accessibility_label(r.label.clone())
                .with_size(Size::XSmall)
                .checked(a.revision == Some(revision))
                .on_change(cx.listener(move |this, _, _, cx| {
                    if let Some(a) = this.actions_state() {
                        a.revision = Some(revision);
                    }
                    cx.notify();
                })),
        )
        .child(ui::medium(ui::xs(r.label.clone())))
        .child(
            ui::muted(ui::xs(r.images.clone()))
                .font_family(crate::theme::MONO)
                .min_w_0()
                .flex_1()
                .truncate(),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(a) = this.actions_state() {
                a.revision = Some(revision);
            }
            cx.notify();
        }))
}
pub(super) fn render(a: &ActionsState, cx: &mut Context<Inspector>) -> Div {
    let rows_height = if a.editor.revisions.is_empty() {
        0.
    } else {
        a.editor.revisions.len() as f32 * 42. - 8.
    };
    div()
        .mt(px(12.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            gpui_kit::uniform_list(
                "action-revisions",
                a.editor.revisions.len(),
                cx.processor(
                    |this: &mut Inspector, range: std::ops::Range<usize>, _, cx| {
                        let super::detail::Detail::Ready(d) = &this.detail else {
                            return Vec::new();
                        };
                        let Some(a) = &d.actions else {
                            return Vec::new();
                        };
                        range
                            .map(|index| div().h(px(42.)).child(row(a, index, cx)))
                            .collect()
                    },
                ),
            )
            .h(px(rows_height))
            .max_h(a.editor.viewport),
        )
        .child(
            div().flex().child(
                button(Variant::Default, 24., a.revision.is_none())
                    .id("action-rollback-review")
                    .test_support()
                    .aria_label("Review rollback")
                    .child("Review rollback")
                    .when(a.revision.is_some(), |b| {
                        b.on_click(cx.listener(|this, _, window, cx| {
                            this.review_action(
                                kubecove_lib::models::ResourceActionKind::Rollback,
                                window,
                                cx,
                            )
                        }))
                    }),
            ),
        )
}
