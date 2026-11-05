//! `SavedPortForwardForm.svelte`: explicit Service scope and ports, never a Pod preset.
use super::{
    Inspector,
    controls::{Variant, button},
};
use crate::{theme::Tokens, ui};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, TestSupportExt, Window, div, prelude::*, px,
};
use kubecove_lib::models::PortForwardRequest;

pub(super) struct PresetForm {
    editing: Option<usize>,
    label: Entity<InputState>,
    context: Entity<InputState>,
    namespace: Entity<InputState>,
    service: Entity<InputState>,
    remote: Entity<InputState>,
    local: Entity<InputState>,
    error: Option<String>,
}
impl Inspector {
    pub(super) fn open_preset_form(
        &mut self,
        editing: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let saved = editing.and_then(|i| {
            self.persisted
                .saved_forward_presets
                .get(&self.preset_key())
                .and_then(|p| p.get(i))
        });
        let label = saved.map(|p| p.label.clone()).unwrap_or_default();
        let request = saved.map(|p| &p.request);
        let context = request
            .map(|p| p.cluster_context.clone())
            .unwrap_or_else(|| self.scope.context.clone());
        let namespace = request.map(|p| p.namespace.clone()).unwrap_or_default();
        let service = request
            .and_then(|p| p.target_name.clone())
            .unwrap_or_default();
        let remote = request
            .map(|p| p.remote_port.to_string())
            .unwrap_or_default();
        let local = request
            .and_then(|p| p.local_port)
            .map(|p| p.to_string())
            .unwrap_or_default();
        self.sessions.form = Some(PresetForm {
            editing,
            label: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(label)
                    .placeholder("Label")
            }),
            context: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(context)
                    .placeholder("Cluster context")
            }),
            namespace: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(namespace)
                    .placeholder("Namespace")
            }),
            service: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(service)
                    .placeholder("Service name")
            }),
            remote: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(remote)
                    .placeholder("Service port")
            }),
            local: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(local)
                    .placeholder("Local port (auto)")
            }),
            error: None,
        });
        cx.notify();
    }
    /// `SavedPortForwardForm.svelte`: rounded-md border bg-muted/20 p-3, grid gap-3 md:grid-cols-3.
    pub(super) fn preset_form(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.sessions.form.as_ref()?;
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .p(px(12.))
                .mx(px(12.))
                .rounded(px(6.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::muted().opacity(0.2))
                .child(
                    div().grid().grid_cols(3).gap(px(12.)).children(
                        [
                            ("Saved forward label", "preset-label", &form.label),
                            (
                                "Saved forward cluster context",
                                "preset-context",
                                &form.context,
                            ),
                            (
                                "Saved forward namespace",
                                "preset-namespace",
                                &form.namespace,
                            ),
                            (
                                "Saved forward Service name",
                                "preset-service",
                                &form.service,
                            ),
                            ("Saved forward Service port", "preset-remote", &form.remote),
                            ("Saved forward local port", "preset-local", &form.local),
                        ]
                        .into_iter()
                        .map(|(label, id, input)| Input::new(input).id(id).aria_label(label)),
                    ),
                )
                .when_some(form.error.clone(), |card, error| {
                    card.child(ui::xs(error).text_color(Tokens::destructive()))
                })
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            button(Variant::Outline, 28., false)
                                .id("preset-cancel")
                                .test_support()
                                .child(ui::xs("Cancel"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sessions.form = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            button(Variant::Default, 28., false)
                                .id("preset-save")
                                .test_support()
                                .child(ui::xs(if form.editing.is_some() {
                                    "Save changes"
                                } else {
                                    "Save forward"
                                }))
                                .on_click(cx.listener(|this, _, _, cx| this.submit_preset(cx))),
                        ),
                )
                .into_any_element(),
        )
    }
    fn submit_preset(&mut self, cx: &mut Context<Self>) {
        let preset_key = self.preset_key();
        let env = self.session_env_var();
        let Some(form) = &mut self.sessions.form else {
            return;
        };
        let context = form.context.read(cx).value().trim().to_owned();
        let namespace = form.namespace.read(cx).value().trim().to_owned();
        let service = form.service.read(cx).value().trim().to_owned();
        let remote = form
            .remote
            .read(cx)
            .value()
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|p| *p != 0);
        let local = form.local.read(cx).value();
        let port = local.trim().parse::<u16>().ok().filter(|p| *p >= 1024);
        if context.is_empty()
            || namespace.is_empty()
            || service.is_empty()
            || remote.is_none()
            || (!local.trim().is_empty() && port.is_none())
        {
            form.error = Some("Cluster context, namespace, Service name and a TCP port are required. Local port must be 1024 or higher when set.".into());
            cx.notify();
            return;
        }
        let request = PortForwardRequest {
            cluster_context: context,
            kubeconfig_env_var: env,
            namespace,
            target_kind: Some("Service".into()),
            target_name: Some(service),
            pod_name: None,
            remote_port: remote.unwrap().into(),
            local_port: port.map(i64::from),
        };
        let presets = self
            .persisted
            .saved_forward_presets
            .entry(preset_key)
            .or_default();
        let saved = crate::preferences::SavedForward {
            request,
            label: form.label.read(cx).value().trim().to_owned(),
        };
        match form.editing {
            Some(i) => presets[i] = saved,
            None => presets.push(saved),
        }
        self.sessions.form = None;
        self.save_preferences(cx);
        cx.notify();
    }
}
