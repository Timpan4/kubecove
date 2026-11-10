//! Saved forwards retain their exact-target controls in the session table.
use super::{
    Inspector,
    port_forward_rows::{table_row, text, xs_button},
};
use crate::targets::TestSupportExt;
use gpui_kit::{Context, Div, SharedString, div, prelude::*, px};
impl Inspector {
    pub(super) fn preset_table_rows(&self, cx: &mut Context<Self>) -> Option<Div> {
        let rows: Vec<Vec<gpui_kit::AnyElement>> = self
            .persisted
            .saved_forward_presets
            .get(&self.preset_key())
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(index, saved)| {
                let preset = &saved.request;
                vec![
                    div()
                        .id(SharedString::from(format!("preset:{index}")))
                        .test_support()
                        .aria_label(saved.label.clone())
                        .child(text(if saved.label.is_empty() {
                            format!(
                                "{}:{}",
                                preset.target_name.as_deref().unwrap_or_default(),
                                preset.remote_port
                            )
                        } else {
                            saved.label.clone()
                        }))
                        .into_any_element(),
                    text(preset.cluster_context.clone()),
                    text(preset.namespace.clone()),
                    text(preset.target_name.clone().unwrap_or_default()),
                    text(
                        preset
                            .local_port
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "auto".into()),
                    ),
                    text("saved"),
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .child(
                            xs_button("play", "Start", false)
                                .id(SharedString::from(format!("start-preset:{index}")))
                                .test_support()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.start_saved_forward(index, window, cx)
                                })),
                        )
                        .child(
                            xs_button("pencil", "Edit", false)
                                .id(SharedString::from(format!("edit-preset:{index}")))
                                .test_support()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_preset_form(Some(index), window, cx)
                                })),
                        )
                        .child(
                            xs_button("x", "Delete", true)
                                .id(SharedString::from(format!("remove-preset:{index}")))
                                .test_support()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let preset_key = this.preset_key();
                                    let Some(presets) = this
                                        .persisted
                                        .saved_forward_presets
                                        .get_mut(&preset_key)
                                        .filter(|presets| index < presets.len())
                                    else {
                                        return;
                                    };
                                    presets.remove(index);
                                    this.preset_removed(&preset_key, index);
                                    this.save_preferences(cx);
                                    cx.notify();
                                })),
                        )
                        .into_any_element(),
                ]
            })
            .collect();
        let last = rows.len().checked_sub(1)?;
        Some(
            div().children(
                rows.into_iter()
                    .enumerate()
                    .map(|(index, row)| table_row(row, index == last)),
            ),
        )
    }
}
