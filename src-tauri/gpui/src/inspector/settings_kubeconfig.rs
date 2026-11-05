//! The Kubeconfig settings category, mirroring `KubeconfigSettings.svelte`.
//! Sources read and write through the backend's typed commands; when the
//! backend cannot save them, the controls render disabled with the reason.

use super::Inspector;
use super::controls::{Variant, button};
use super::settings_page::{Observed, settings_row, switch};
use crate::backend::KubeconfigSourcesSummary;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{Context, Div, SharedString, Window, div, prelude::*, px};

impl Inspector {
    fn kubeconfig_path_entry(
        &self,
        index: usize,
        entry: &kubecove_lib::commands::kubeconfig::KubeconfigPathEntry,
        count: usize,
        cx: &mut Context<Self>,
    ) -> Observed {
        let disabled = self.settings.busy || self.backend.kubeconfig_writes_unavailable().is_some();
        let path = entry.path.clone();
        div()
            .id(SharedString::from(format!("kubeconfig-path:{path}")))
            .test_support()
            .aria_label(path.clone())
            .flex()
            .items_center()
            .gap(px(8.))
            .mt(px(8.))
            .p(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(Tokens::border())
            .bg(Tokens::background())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        ui::medium(ui::xs(
                            path.rsplit(['/', '\\']).next().unwrap_or(&path).to_owned(),
                        ))
                        .truncate(),
                    )
                    .child(ui::muted(ui::xs(path.clone())).truncate()),
            )
            .children(
                [
                    ("Move kubeconfig path up", "arrow-up", index.checked_sub(1)),
                    (
                        "Move kubeconfig path down",
                        "arrow-down",
                        (index + 1 < count).then_some(index + 1),
                    ),
                ]
                .into_iter()
                .map(|(label, icon, target)| {
                    button(Variant::Ghost, 24., disabled || target.is_none())
                        .id(SharedString::from(label))
                        .test_support()
                        .aria_label(label)
                        .child(ui::icon(icon, 14., Tokens::foreground()))
                        .when(!disabled && target.is_some(), |button| {
                            button.on_click(cx.listener(move |this, _, window, cx| {
                                let Some(Ok(sources)) = &this.settings.sources else {
                                    return;
                                };
                                let mut paths: Vec<_> = sources
                                    .paths
                                    .iter()
                                    .map(|entry| entry.path.clone())
                                    .collect();
                                let Some(target) = target
                                    .filter(|target| *target < paths.len() && index < paths.len())
                                else {
                                    return;
                                };
                                paths.swap(index, target);
                                let read = this.backend.reorder_kubeconfig_paths(paths);
                                this.run_kubeconfig(read, window, cx);
                            }))
                        })
                }),
            )
            .child(
                button(Variant::Ghost, 24., disabled)
                    .id("settings-button:Remove kubeconfig path")
                    .test_support()
                    .aria_label("Remove kubeconfig path")
                    .child(ui::icon("trash-2", 14., Tokens::foreground()))
                    .when(!disabled, |button| {
                        button.on_click(cx.listener(move |this, _, window, cx| {
                            let read = this.backend.remove_kubeconfig_path(path.clone());
                            this.run_kubeconfig(read, window, cx);
                        }))
                    }),
            )
    }
    /// `runSourceUpdate`: applies the returned sources and refills the draft.
    pub(super) fn run_kubeconfig(
        &mut self,
        read: crate::backend::Read<KubeconfigSourcesSummary>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.busy = true;
        self.settings.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let state = &mut this.settings;
                state.busy = false;
                match result {
                    Ok(sources) => {
                        let value = sources.kubeconfig_env_var.clone();
                        state
                            .env_var
                            .update(cx, |input, cx| input.set_value(value, window, cx));
                        state.sources = Some(Ok(sources));
                    }
                    Err(error) => state.sources = Some(Err(error.message)),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn save_env_var(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.settings.env_var.read(cx).value().to_string();
        let read = self.backend.set_kubeconfig_env_var(value);
        self.run_kubeconfig(read, window, cx);
    }

    fn reset_env_var(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let read = self.backend.set_kubeconfig_env_var(String::new());
        self.run_kubeconfig(read, window, cx);
    }

    fn toggle_source_labels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Ok(sources)) = &self.settings.sources else {
            return;
        };
        let read = self
            .backend
            .set_show_kubeconfig_source_labels(!sources.show_source_labels);
        self.run_kubeconfig(read, window, cx);
    }

    pub(super) fn kubeconfig_section(&self, cx: &mut Context<Self>) -> Div {
        let state = &self.settings;
        let unavailable = self.backend.kubeconfig_writes_unavailable();
        let disabled = unavailable.is_some() || state.busy;
        let (label, description, show, error) = match &state.sources {
            None => (
                "KUBECONFIG".to_owned(),
                "Loading kubeconfig sources.".to_owned(),
                false,
                None,
            ),
            Some(Ok(sources)) => (
                sources.source_label.clone(),
                format!(
                    "{}. Full file paths are shown only here.",
                    sources.source_label
                ),
                sources.show_source_labels,
                None,
            ),
            Some(Err(error)) => (
                "KUBECONFIG".to_owned(),
                "Loading kubeconfig sources.".to_owned(),
                false,
                Some(error.clone()),
            ),
        };
        let reason = |element: Observed| -> Observed {
            match unavailable {
                Some(reason) => element
                    .aria_description(reason)
                    .opacity(0.5)
                    .tooltip(move |window, cx| Tooltip::new(reason).build(window, cx)),
                None => element,
            }
        };
        let save = reason(
            button(Variant::Outline, 24., false)
                .id("kubeconfig-env-save")
                .test_support()
                .aria_label("Save")
                .child("Save"),
        )
        .when(!disabled, |save| {
            save.on_click(cx.listener(|this, _, window, cx| this.save_env_var(window, cx)))
        });
        let reset = reason(
            div()
                .id("kubeconfig-env-reset")
                .test_support()
                .aria_label("Reset kubeconfig env var")
                .child(ui::ghost_icon("rotate-ccw", 24., 14.)),
        )
        .when(!disabled, |reset| {
            reset.on_click(cx.listener(|this, _, window, cx| this.reset_env_var(window, cx)))
        });
        let switch =
            reason(switch("Show kubeconfig source labels", show)).when(!disabled, |switch| {
                switch.on_click(
                    cx.listener(|this, _, window, cx| this.toggle_source_labels(window, cx)),
                )
            });
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(settings_row(
                "Active sources",
                description,
                ui::outline_badge(label).into_any_element(),
            ))
            .children(
                state
                    .sources
                    .as_ref()
                    .and_then(|sources| sources.as_ref().ok())
                    .into_iter()
                    .flat_map(|sources| &sources.warnings)
                    .enumerate()
                    .map(|(index, warning)| {
                        ui::xs(format!(
                            "{}{}",
                            warning
                                .path
                                .as_ref()
                                .map_or(String::new(), |path| format!("{path}: ")),
                            warning.message
                        ))
                        .p(px(12.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(Tokens::destructive().opacity(0.4))
                        .id(SharedString::from(format!("source-warning:{index}")))
                        .role(gpui_kit::Role::Alert)
                    }),
            )
            .when_some(error, |section, error| {
                section.child(
                    ui::xs(error.clone())
                        .id("kubeconfig-error")
                        .test_support()
                        .aria_label("Kubeconfig sources failed")
                        .aria_description(error)
                        .role(gpui_kit::Role::Alert)
                        .text_color(Tokens::destructive()),
                )
            })
            .child(settings_row(
                "Environment variable",
                "Variable holding kubeconfig paths. Default is read when unset.".into(),
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div().w(px(192.)).child(
                            Input::new(&state.env_var)
                                .id("kubeconfig-env-var")
                                .aria_label("Kubeconfig env var")
                                .disabled(unavailable.is_some())
                                .text_xs()
                                .h(px(32.)),
                        ),
                    )
                    .child(save)
                    .child(reset)
                    .into_any_element(),
            ))
            .child(settings_row(
                "Show source labels",
                "Marks contexts with kubeconfig source they were discovered from.".into(),
                switch.into_any_element(),
            ))
            .child(
                self.added_paths(
                    state
                        .sources
                        .as_ref()
                        .and_then(|sources| sources.as_ref().ok()),
                    cx,
                ),
            )
    }
    /// "Added kubeconfig paths": adding files needs a native file dialog.
    fn added_paths(
        &self,
        sources: Option<&KubeconfigSourcesSummary>,
        cx: &mut Context<Self>,
    ) -> Observed {
        let paths = sources.map_or(&[][..], |sources| sources.paths.as_slice());
        div()
        .id("setting-row:Added kubeconfig paths")
        .test_support()
        .p(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(Tokens::border())
        .bg(Tokens::card())
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(ui::medium(ui::sm("Added kubeconfig paths")))
                        .child(ui::muted(ui::xs(
                            "Extra kubeconfig files KubeCove reads beside environment variable.",
                        ))),
                )
                .child(
                    button(Variant::Outline, 24., self.backend.kubeconfig_writes_unavailable().is_some() || self.settings.busy)
                        .id("settings-button:Choose files").test_support().aria_label("Choose files")
                        .child(ui::icon("plus", 12., Tokens::foreground()))
                        .child("Choose files")
                        .when(self.backend.kubeconfig_writes_unavailable().is_none() && !self.settings.busy, |button| button.on_click(cx.listener(|this, _, window, cx| {
                            let read = this.backend.pick_kubeconfig_paths(); this.run_kubeconfig(read, window, cx);
                        }))),
                ),
        )
        .children(paths.iter().enumerate().map(|(index, entry)| self.kubeconfig_path_entry(index, entry, paths.len(), cx)))
        .when(paths.is_empty(), |panel| {
            panel.child(
                ui::muted(ui::xs("No app-added kubeconfig paths."))
                    .mt(px(12.))
                    .p(px(12.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Tokens::border())
                    .bg(Tokens::background()),
            )
        })
        .child(div().mt(px(8.)).child(Textarea::new(&self.settings.pasted_paths).aria_label("Kubeconfig paths").disabled(self.backend.kubeconfig_writes_unavailable().is_some()).text_xs().h(px(64.))))
        .child(div().flex().justify_end().mt(px(8.)).child(button(Variant::Outline, 24., self.settings.busy || self.backend.kubeconfig_writes_unavailable().is_some() || self.settings.pasted_paths.read(cx).value().trim().is_empty())
            .id("settings-button:Add pasted paths").test_support().aria_label("Add pasted paths")
            .child(ui::icon("plus",12.,Tokens::foreground())).child("Add pasted paths")
            .when(self.backend.kubeconfig_writes_unavailable().is_none() && !self.settings.busy, |button| button.on_click(cx.listener(|this, _, window, cx| {
                let value = this.settings.pasted_paths.read(cx).value().to_string();
                if value.trim().is_empty() { return; }
                let read = this.backend.add_kubeconfig_paths(vec![value]); this.run_kubeconfig(read,window,cx);
            })))))
    }
}
