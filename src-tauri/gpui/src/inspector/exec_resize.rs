//! Terminal fit calls the same narrow resize command as `ExecSessionTerminal.svelte`.
use super::{Inspector, exec_state::ExecPhase, request::InFlight};
use gpui_kit::Context;
use kubecove_lib::models::PodExecTerminalSize;
use std::num::NonZeroU16;

/// A live session's size updates. One task sends the latest size at a time.
pub(super) struct ExecResize {
    sender: tokio::sync::watch::Sender<PodExecTerminalSize>,
    _task: InFlight,
}

pub(super) fn terminal_columns(width: f32, window: &mut gpui_kit::Window) -> Option<u16> {
    let font = window
        .text_system()
        .resolve_font(&gpui_kit::font(crate::theme::MONO));
    let advance = window
        .text_system()
        .advance(font, gpui_kit::px(12.), 'm')
        .ok()?;
    // `ExecSessionTerminal.svelte`: px-2.5. `podExecHelpers.ts`: dimensions 1..=500.
    Some(
        ((width - 20.) / f32::from(advance.width))
            .floor()
            .clamp(1., 500.) as u16,
    )
}

impl Inspector {
    pub(super) fn resize_exec(&mut self, key: usize, cols: u16, rows: u16, cx: &mut Context<Self>) {
        let Some(tab) = self.exec.tabs.iter_mut().find(|t| t.key == key) else {
            return;
        };
        let (Some(cols_nz), Some(rows_nz)) = (NonZeroU16::new(cols), NonZeroU16::new(rows)) else {
            return;
        };
        if tab.parser.screen().size() == (rows_nz, cols_nz) {
            self.begin_exec_stream(key, cx);
            return;
        }
        tab.selection = None;
        super::exec_reflow::resize(&mut tab.parser, rows_nz, cols_nz);
        tab.refresh_view();
        self.begin_exec_stream(key, cx);
        self.send_exec_resize(key, cx);
        cx.notify();
    }
    pub(super) fn send_exec_resize(&mut self, key: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.exec.tabs.iter_mut().find(|t| t.key == key) else {
            return;
        };
        let (rows, cols) = tab.parser.screen().size();
        let size = PodExecTerminalSize {
            cols: cols.get(),
            rows: rows.get(),
        };
        let ExecPhase::Live(live) = &mut tab.phase else {
            return;
        };
        if let Some(resize) = &live.resize {
            resize.sender.send_replace(size);
            return;
        }
        let id = live.id.clone();
        let (sender, mut receiver) = tokio::sync::watch::channel(size);
        let backend = self.backend.clone();
        let task = cx.spawn(async move |this, cx| {
            loop {
                let size = receiver.borrow_and_update().clone();
                let result = backend.exec_resize(id.clone(), size).await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(tab) = this
                        .exec
                        .tabs
                        .iter_mut()
                        .find(|t| t.key == key && t.session_id() == Some(id.as_str()))
                    {
                        match result {
                            Ok(true) => {}
                            Ok(false) => tab.error = Some("Terminal resize was refused".into()),
                            Err(error) => tab.error = Some(error.message),
                        }
                        cx.notify();
                    }
                });
                if receiver.changed().await.is_err() {
                    break;
                }
            }
        });
        live.resize = Some(ExecResize {
            sender,
            _task: task.into(),
        });
    }
}
