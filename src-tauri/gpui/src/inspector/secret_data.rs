//! Secret values live only in the open detail, as in SecretDataViewer.svelte.
use super::{
    Inspector,
    controls::{Variant, button},
    detail::{Detail, ResourceDetail},
};
use crate::targets::TestSupportExt;
use crate::{theme::Tokens, ui};
use gpui_kit::{Context, Div, SharedString, Task, Window, div, prelude::*, px};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct SecretData {
    values: BTreeMap<String, String>,
    errors: BTreeMap<String, String>,
    tasks: BTreeMap<String, Task<()>>,
}

pub(super) fn view(detail: &ResourceDetail, cx: &mut Context<Inspector>) -> Div {
    let document: serde_json::Value =
        serde_yaml::from_str(&detail.yaml_view.text).unwrap_or_default();
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(ui::medium(ui::sm("Secret data")))
        .child(ui::muted(ui::xs(
            "Values stay masked until revealed. Revealed values clear after 60 seconds.",
        )));
    for source in ["data", "stringData"] {
        let Some(entries) = document[source].as_object() else {
            continue;
        };
        for key in entries.keys() {
            let key = key.clone();
            let id = format!("{source}:{key}");
            let value = detail.secrets.values.get(&id);
            let revealed = value.is_some();
            let available = source == "data" && detail.data.summary.namespace.is_some();
            let action_id = id.clone();
            let action_key = key.clone();
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .p(px(12.))
                    .border_1()
                    .border_color(Tokens::border())
                    .rounded(px(8.))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(ui::xs(format!("{key} ({source})")))
                            .when(available, |row| {
                                row.child(
                                    button(Variant::Outline, 28., false)
                                        .id(SharedString::from(format!("secret-reveal:{id}")))
                                        .test_support()
                                        .aria_label(if revealed { "Hide" } else { "Reveal" })
                                        .child(if revealed { "Hide" } else { "Reveal" })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if revealed {
                                                this.hide_secret(&action_id, cx);
                                            } else {
                                                this.reveal_secret(
                                                    action_id.clone(),
                                                    action_key.clone(),
                                                    window,
                                                    cx,
                                                );
                                            }
                                        })),
                                )
                            }),
                    )
                    .child(ui::xs("••••••").font_family(crate::theme::MONO))
                    .when_some(value.cloned(), |row, value| {
                        row.child(
                            ui::xs(value.clone())
                                .id(SharedString::from(format!("secret-value:{source}:{key}")))
                                .test_support()
                                .aria_label(value)
                                .font_family(crate::theme::MONO),
                        )
                    })
                    .when_some(
                        detail
                            .secrets
                            .errors
                            .get(&format!("{source}:{key}"))
                            .cloned(),
                        |row, message| {
                            row.child(
                                ui::xs(message.clone())
                                    .id(SharedString::from(format!("secret-error:{source}:{key}")))
                                    .test_support()
                                    .aria_label(message)
                                    .text_color(Tokens::destructive()),
                            )
                        },
                    ),
            );
        }
    }
    body
}

impl Inspector {
    pub(super) fn hide_secret(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Detail::Ready(detail) = &mut self.detail {
            detail.secrets.values.remove(id);
            detail.secrets.errors.remove(id);
            detail.secrets.tasks.remove(id);
        }
        cx.notify();
    }
    fn reveal_secret(
        &mut self,
        id: String,
        key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.persisted.preferences.redact_secrets {
            return;
        }
        let Detail::Ready(detail) = &mut self.detail else {
            return;
        };
        detail.secrets.errors.remove(&id);
        let read = self.backend.reveal_secret(detail.data.summary.clone(), key);
        let target = id.clone();
        // The detail owns this task, so it only settles into the detail that started it.
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let shown = this
                .update(cx, |this, cx| {
                    if this.persisted.preferences.redact_secrets {
                        return false;
                    }
                    let Detail::Ready(detail) = &mut this.detail else {
                        return false;
                    };
                    match result {
                        Ok(encoded) => match crate::secret_values::decode(&encoded) {
                            Ok(value) => {
                                detail.secrets.values.insert(target.clone(), value);
                            }
                            Err(error) => {
                                detail.secrets.errors.insert(target.clone(), error.into());
                            }
                        },
                        // Provider errors can contain response bodies. Never retain them here.
                        Err(_) => {
                            detail
                                .secrets
                                .errors
                                .insert(target.clone(), "Unable to reveal value".into());
                        }
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if shown {
                // SecretDataViewer.svelte uses the same 60-second lifetime.
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(60))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if let Detail::Ready(detail) = &mut this.detail {
                        detail.secrets.values.remove(&target);
                    }
                    cx.notify();
                });
            }
        });
        detail.secrets.tasks.insert(id, task);
    }
}
