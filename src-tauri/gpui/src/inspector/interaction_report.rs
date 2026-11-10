//! `InteractionRecorderSettings.svelte`: the record switch and the report with
//! Clear and Copy. Recording and its output shape live in `crate::interactions`.
use super::{
    Inspector,
    controls::{Variant, button},
    settings_page::{settings_row, switch},
};
use crate::{interactions::Interaction, settings::Row, targets::TestSupportExt, theme::Tokens, ui};
use gpui_kit::{AnyElement, ClipboardItem, Context, SharedString, div, prelude::*, px};

const HEADERS: [&str; 8] = [
    "Time",
    "Kind",
    "Target",
    "Dropped",
    "Max frame",
    "To paint",
    "DOM",
    "Top source",
];
// The web's table shows this many rows, `interactionTableRows`' default.
const MAX_ROWS: usize = 10;

fn ms(value: f64) -> String {
    format!("{value} ms")
}

/// `interactionTableRows`: interactions with dropped frames, worst first.
fn table_rows<'a>(interactions: impl Iterator<Item = &'a Interaction>) -> Vec<[String; 8]> {
    let mut dropped: Vec<_> = interactions
        .filter(|interaction| interaction.dropped_frames() > 0)
        .collect();
    dropped.sort_by(|a, b| {
        b.dropped_frames()
            .cmp(&a.dropped_frames())
            .then(b.max_frame_ms().total_cmp(&a.max_frame_ms()))
    });
    dropped
        .into_iter()
        .take(MAX_ROWS)
        .map(|interaction| {
            [
                interaction.local_time(),
                interaction.kind.label().to_owned(),
                interaction.target.clone(),
                interaction.dropped_frames().to_string(),
                ms(interaction.max_frame_ms()),
                ms(interaction.max_input_to_paint_ms()),
                // GPUI has no DOM; see `crate::interactions`.
                "+0 -0".to_owned(),
                interaction
                    .sources()
                    .first()
                    .map_or("-".to_owned(), |(source, _)| (*source).to_owned()),
            ]
        })
        .collect()
}

impl Inspector {
    pub(super) fn interaction_switch(
        &self,
        row: &'static Row,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let recording = self.presentation.recorder.borrow().recording();
        settings_row(
            row.title,
            row.description.into(),
            switch(row.title, recording)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_interaction_recording(!recording, window);
                    cx.notify();
                }))
                .into_any_element(),
        )
        .into_any_element()
    }

    pub(super) fn interaction_report(
        &self,
        row: &'static Row,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let recorder = self.presentation.recorder.borrow();
        let rows = table_rows(recorder.interactions());
        let controls = div()
            .flex()
            .gap(px(8.))
            .child(
                button(Variant::Outline, 24., false)
                    .id("interactions-clear")
                    .test_support()
                    .aria_label("Clear")
                    .child(ui::icon("trash-2", 14., Tokens::foreground()))
                    .child("Clear")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.presentation.recorder.borrow_mut().clear();
                        this.presentation.recorder_status = Some("Interactions cleared.".into());
                        cx.notify();
                    })),
            )
            .child(
                button(Variant::Default, 24., false)
                    .id("interactions-copy")
                    .test_support()
                    .aria_label("Copy")
                    .child(ui::icon("copy", 14., Tokens::primary_foreground()))
                    .child("Copy")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let (lines, count) = {
                            let recorder = this.presentation.recorder.borrow();
                            (recorder.json_lines(), recorder.len())
                        };
                        cx.write_to_clipboard(ClipboardItem::new_string(lines));
                        this.presentation.recorder_status =
                            Some(format!("Copied {count} interactions."));
                        cx.notify();
                    })),
            );
        let count = format!("{} interactions recorded.", recorder.len());
        let mut table = div()
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .child(
                div()
                    .flex()
                    .py(px(8.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .children(HEADERS.map(|label| {
                        ui::semibold(ui::muted(ui::xs(label.to_uppercase())))
                            .flex_1()
                            .px(px(12.))
                    })),
            );
        if rows.is_empty() {
            let empty = "No interactions with dropped frames yet.";
            table = table.child(
                ui::muted(ui::sm(empty))
                    .py(px(32.))
                    .text_center()
                    .id("interactions-empty")
                    .test_support()
                    .aria_label(empty),
            );
        }
        for (index, cells) in rows.into_iter().enumerate() {
            table = table.child(
                div()
                    .flex()
                    .py(px(8.))
                    .id(("interaction-row", index))
                    .test_support()
                    .aria_label(cells.join(" · "))
                    .children(
                        cells.map(|value| ui::xs(value).flex_1().min_w_0().truncate().px(px(12.))),
                    ),
            );
        }
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .mt(px(16.))
            .child(
                ui::muted(ui::xs(count.clone()))
                    .id("interactions-count")
                    .test_support()
                    .aria_label(count),
            )
            .child(table);
        if let Some(status) = &self.presentation.recorder_status {
            body = body.child(
                ui::muted(ui::xs(status.clone()))
                    .id("interactions-status")
                    .test_support()
                    .aria_label(status.clone()),
            );
        }
        div()
            .id(SharedString::from(format!("setting-row:{}", row.title)))
            .test_support()
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::card())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .min_w_0()
                            .child(ui::medium(ui::sm(row.title)))
                            .child(ui::muted(ui::xs(row.description)).mt(px(4.))),
                    )
                    .child(controls),
            )
            .child(body)
            .into_any_element()
    }
}
