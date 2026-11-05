//! `ExecSessionMenu.svelte`: argv lines are passed literally, without a shell.
use super::{
    Inspector,
    controls::{Variant, button},
    detail::Detail,
};
use crate::{theme::Tokens, ui};
use gpui_kit::component::{
    IndexPath,
    input::{Enter, InputEvent, Textarea, TextareaState},
    select::{SearchableVec, Select, SelectEvent, SelectState},
};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, SharedString, Subscription, TestSupportExt,
    Window, div, prelude::*, px,
};

pub(super) struct ExecCustomForm {
    argv: Entity<TextareaState>,
    container: Entity<SelectState<SearchableVec<SharedString>>>,
    _subscriptions: Vec<Subscription>,
}
impl Inspector {
    pub(super) fn open_exec_custom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Detail::Ready(detail) = &self.detail else {
            return;
        };
        let mut names: Vec<SharedString> = detail
            .data
            .status
            .as_ref()
            .into_iter()
            .flat_map(|s| {
                ["containerStatuses", "ephemeralContainerStatuses"]
                    .into_iter()
                    .flat_map(|key| s[key].as_array().into_iter().flatten())
            })
            .filter_map(|c| c["name"].as_str().map(Into::into))
            .collect();
        if names.is_empty() {
            names.push("".into());
        }
        let container = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(names),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        // The web textarea declares rows=3.
        let argv = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder("/usr/bin/env\nprintenv")
        });
        let subscriptions = vec![
            cx.subscribe(&argv, |_, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(
                &container,
                |_, _, _: &SelectEvent<SearchableVec<SharedString>>, cx| cx.notify(),
            ),
        ];
        self.exec.custom = Some(ExecCustomForm {
            argv,
            container,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }
    /// `runCustom`: starts the argv in the chosen container. An empty argv does nothing.
    fn run_custom(
        &mut self,
        container: String,
        argv: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if argv.is_empty() {
            return;
        }
        self.start_exec(container, argv, window, cx);
        self.exec.custom = None;
    }

    pub(super) fn exec_custom_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.exec.custom.as_ref()?;
        let argv: Vec<String> = form
            .argv
            .read(cx)
            .value()
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let container = form
            .container
            .read(cx)
            .selected_value()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let executable = argv
            .first()
            .map(|s| s.rsplit('/').next().unwrap_or(s))
            .unwrap_or("command");
        let run_label = format!(
            "Run {executable} in {}",
            if container.is_empty() {
                "default container"
            } else {
                &container
            }
        );
        let (key_container, key_argv) = (container.clone(), argv.clone());
        let argv_focus = form.argv.read(cx).focus_handle(cx);
        Some(
            div()
                .w(px(256.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(10.))
                .bg(Tokens::card())
                .border_1()
                .border_color(Tokens::border())
                .rounded(px(6.))
                .capture_action(cx.listener(move |this, action: &Enter, window, cx| {
                    if argv_focus.is_focused(window) && action.secondary {
                        cx.stop_propagation();
                        this.run_custom(key_container.clone(), key_argv.clone(), window, cx);
                    }
                }))
                .child(ui::muted(ui::xs("Custom command")))
                .child(ui::muted(ui::xs("Container")))
                .child(
                    Select::new(&form.container)
                        .id("exec-custom-container")
                        .h(px(28.)),
                )
                .child(ui::muted(ui::xs("Arguments, one per line")))
                .child(Textarea::new(&form.argv).aria_label("Exec arguments"))
                .child(ui::muted(ui::xs("Passed as-is. No shell parsing.")))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            button(Variant::Ghost, 28., false)
                                .id("exec-custom-back")
                                .child(ui::xs("Back"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.exec.custom = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            button(Variant::Default, 28., argv.is_empty())
                                .id("exec-custom-run")
                                .test_support()
                                .aria_label(run_label.clone())
                                .child(ui::xs(run_label))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.run_custom(container.clone(), argv.clone(), window, cx);
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}
