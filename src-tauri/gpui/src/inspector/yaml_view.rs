//! YAML shape and encoding controls from YamlTab.svelte. Reads and formatting
//! happen on changes, never while rendering a frame.
use super::{
    Inspector,
    detail::Detail,
    yaml_apply::{Feedback, Phase},
};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_kit::{AppContext, Context, Entity, SharedString, Subscription, Window, prelude::*, px};
use kubecove_lib::models::{AppError, AppErrorKind, YamlEncoding, YamlViewMode};

type Choices = SelectState<SearchableVec<SharedString>>;
pub(super) struct YamlView {
    pub shape: Entity<Choices>,
    pub encoding: Entity<Choices>,
    pub mode: YamlViewMode,
    pub format: YamlEncoding,
    pub text: String,
    pub error: Option<String>,
    _subscriptions: Vec<Subscription>,
}
fn choices(labels: &[&str], window: &mut Window, cx: &mut Context<Inspector>) -> Entity<Choices> {
    cx.new(|cx| {
        SelectState::new(
            SearchableVec::new(
                labels
                    .iter()
                    .map(|s| SharedString::from(s.to_string()))
                    .collect::<Vec<_>>(),
            ),
            Some(gpui_kit::component::IndexPath::new(0)),
            window,
            cx,
        )
    })
}
impl YamlView {
    pub fn new(window: &mut Window, cx: &mut Context<Inspector>) -> Self {
        let shape = choices(&["Kubectl view", "Apply clean"], window, cx);
        let encoding = choices(&["YAML", "KYAML"], window, cx);
        let subscriptions = vec![
            cx.subscribe_in(&shape, window, |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Detail::Ready(d) = &mut this.detail {
                        if d.yaml.editing() {
                            return;
                        }
                        d.yaml_view.mode = if value.as_ref() == "Apply clean" {
                            YamlViewMode::ApplyClean
                        } else {
                            YamlViewMode::Kubectl
                        };
                    }
                    this.reload_yaml_view(window, cx);
                }
            }),
            cx.subscribe_in(&encoding, window, |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Detail::Ready(d) = &mut this.detail {
                        if d.yaml.editing() {
                            return;
                        }
                        d.yaml_view.format = if value.as_ref() == "KYAML" {
                            YamlEncoding::Kyaml
                        } else {
                            YamlEncoding::Yaml
                        };
                    }
                    this.reload_yaml_view(window, cx);
                }
            }),
        ];
        Self {
            shape,
            encoding,
            mode: Default::default(),
            format: Default::default(),
            text: String::new(),
            error: None,
            _subscriptions: subscriptions,
        }
    }
}

pub(super) fn picker(view: &YamlView, shape: bool, disabled: bool) -> impl IntoElement {
    let (id, label, state, width) = if shape {
        ("yaml-shape", "YAML shape", &view.shape, 144.)
    } else {
        ("yaml-encoding", "YAML encoding", &view.encoding, 112.)
    };
    gpui_kit::div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .child(crate::ui::medium(crate::ui::muted(crate::ui::text(
            if shape { "YAML shape" } else { "Encoding" },
            11.,
            16.,
        ))))
        .child(
            Select::new(state)
                .id(id)
                .accessibility_label(label)
                .disabled(disabled)
                .w(px(width))
                .h(px(28.)),
        )
}

impl Inspector {
    pub(super) fn reload_yaml_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let Some(d) = self.yaml_apply() else {
            return;
        };
        if d.yaml.editing() {
            return;
        }
        d.yaml_view.error = None;
        d.secrets = Default::default();
        let read = backend.resource_yaml_encoded(
            d.data.summary.clone(),
            d.yaml_view.mode,
            d.yaml_view.format,
        );
        let read = self.timings.measure("get_resource_yaml", read);
        self.diff_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Some(d) = this.yaml_apply() {
                    match result {
                        Ok(text) => {
                            d.yaml_view.text = text.clone();
                            d.editor.update(cx, |e, cx| e.set_value(text, window, cx));
                            d.marks.refresh(&d.editor, cx);
                        }
                        Err(e) => d.yaml_view.error = Some(e.message),
                    }
                }
                this.refresh_yaml_lens(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn format_yaml(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.yaml_apply() else {
            return;
        };
        if !matches!(d.yaml.phase, Phase::Editing | Phase::Reviewed(_)) {
            return;
        }
        let text = d.editor.read(cx).value().to_string();
        let format = d.yaml_view.format;
        let before = text.clone();
        let work = cx.background_executor().spawn(async move {
            let value: serde_json::Value = serde_yaml::from_str(&text)
                .map_err(|e| AppError::new(e.to_string(), AppErrorKind::Serialization))?;
            kubecove_lib::commands::helpers::serialize_json_value_document(&value, format)
        });
        self.diff_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = work.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(d) = this.yaml_apply() else {
                    return;
                };
                if !d.yaml.editing() || d.editor.read(cx).value().as_ref() != before {
                    return;
                }
                let mut formatted = false;
                match result {
                    Ok(text) => {
                        d.yaml.draft_changed();
                        d.editor.update(cx, |e, cx| e.set_value(text, window, cx));
                        d.marks.refresh(&d.editor, cx);
                        formatted = true;
                    }
                    Err(e) => d.yaml.feedback = Feedback::FormatFailed(e.message),
                }
                // `set_value` emits no change event, so the formatted draft is linted here.
                if formatted {
                    this.schedule_yaml_lint(window, cx);
                } else {
                    this.refresh_yaml_lens(cx);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
