//! The Kubeconfig settings category, mirroring `KubeconfigSettings.svelte`.
//! Sources read and write through the backend's typed commands; when the
//! backend cannot save them, the controls render disabled with the reason.

use super::Inspector;
use super::controls::{Variant, button};
use super::settings_page::{Observed, settings_reason, settings_row, switch, unavailable_control};
use crate::backend::KubeconfigSourcesSummary;
use crate::theme::Tokens;
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::component::input::Input;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{Context, Div, SharedString, Window, div, prelude::*, px};

impl Inspector {
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
                    ui::xs(error)
                        .id("kubeconfig-error")
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
            .child(added_paths(
                state
                    .sources
                    .as_ref()
                    .and_then(|sources| sources.as_ref().ok()),
            ))
    }
}

/// "Added kubeconfig paths": adding files needs a native file dialog.
fn added_paths(sources: Option<&KubeconfigSourcesSummary>) -> Observed {
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
                .child(unavailable_control(
                    "kubeconfig-choose-files",
                    settings_reason("Added kubeconfig paths"),
                    button(Variant::Outline, 24., false)
                        .child(ui::icon("plus", 12., Tokens::foreground()))
                        .child("Choose files"),
                )),
        )
        .children(paths.iter().map(|entry| {
            div()
                .mt(px(8.))
                .p(px(8.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::background())
                .child(
                    ui::medium(ui::xs(
                        entry
                            .path
                            .rsplit(['/', '\\'])
                            .next()
                            .unwrap_or(&entry.path)
                            .to_owned(),
                    ))
                    .truncate(),
                )
                .child(ui::muted(ui::xs(entry.path.clone())).truncate())
        }))
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
}
