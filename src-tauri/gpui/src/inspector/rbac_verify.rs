//! The "Verify exact action" SubjectAccessReview form from `RbacView.svelte`
//! and its latest result.

use super::Inspector;
use super::controls::{Variant, button};
use super::rbac_inspector::caps;
use super::rbac_page::IdentitySource;
use crate::rbac::permissions::{Identity, IdentityKind, identity_defaults, unique_groups};
use crate::theme::{Palette, Tokens};
use crate::ui;
use gpui_kit::TestSupportExt;
use gpui_kit::component::input::{Input, InputEvent, InputState, TextareaState};
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, Subscription, Task, Window, div, prelude::*, px,
};
use kubecove_lib::models::{
    RbacAccessReviewOutcome, RbacAccessReviewRequest, RbacAccessReviewResult,
    RbacAccessReviewTarget, RbacInspectionSummary,
};
use std::rc::Rc;

struct Submitted {
    timestamp: String,
    identity: String,
    target: String,
}

enum Review {
    None,
    Checking(Submitted),
    Done(Submitted, RbacAccessReviewResult),
    Failed(Submitted, String),
}

/// The verifier form and review note, as `RbacView.svelte` keeps them.
pub(super) struct Verifier {
    manual_kind: IdentityKind,
    name: Entity<InputState>,
    namespace: Entity<InputState>,
    groups: Entity<InputState>,
    non_resource: bool,
    verb: Entity<InputState>,
    resource: Entity<InputState>,
    api_group: Entity<InputState>,
    target_namespace: Entity<InputState>,
    subresource: Entity<InputState>,
    resource_name: Entity<InputState>,
    url: Entity<InputState>,
    review: Review,
    pub(super) note: Entity<TextareaState>,
    pub(super) note_error: Option<String>,
    request: Rc<()>,
    task: Option<Task<()>>,
}

impl Verifier {
    pub(super) fn new(
        window: &mut Window,
        cx: &mut Context<Inspector>,
        subscriptions: &mut Vec<Subscription>,
    ) -> Self {
        let mut field = |value: &str, placeholder: &str, resets: bool| {
            let input = cx.new(|cx| {
                let mut input = InputState::new(window, cx).placeholder(placeholder.to_owned());
                input.set_value(value.to_owned(), window, cx);
                input
            });
            if resets {
                subscriptions.push(cx.subscribe_in(&input, window, |this, _, event, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.rbac.verifier.reset();
                        cx.notify();
                    }
                }));
            }
            input
        };
        Self {
            manual_kind: IdentityKind::User,
            name: field("", "Identity name", true),
            namespace: field("default", "Namespace", true),
            groups: field("system:authenticated", "Groups, comma-separated", true),
            non_resource: false,
            verb: field("get", "Verb", true),
            resource: field("pods", "Resource", true),
            api_group: field("", "API group (core)", true),
            target_namespace: field("", "Namespace (cluster if blank)", true),
            subresource: field("", "Subresource", true),
            resource_name: field("", "Resource name", true),
            url: field("/healthz", "/path", true),
            review: Review::None,
            note: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder("Why is this access expected or anomalous?")
            }),
            note_error: None,
            request: Rc::new(()),
            task: None,
        }
    }

    /// Any input change drops a result for an older request.
    pub(super) fn reset(&mut self) {
        self.review = Review::None;
        self.request = Rc::new(());
        self.task = None;
    }

    /// `manualIdentityValue`.
    pub(super) fn manual_identity(&self, cx: &App) -> Identity {
        let text = |input: &Entity<InputState>| input.read(cx).value().trim().to_owned();
        Identity {
            kind: self.manual_kind,
            name: text(&self.name),
            namespace: (self.manual_kind == IdentityKind::ServiceAccount)
                .then(|| text(&self.namespace)),
            groups: if self.manual_kind == IdentityKind::User {
                unique_groups(&self.groups.read(cx).value())
            } else {
                vec![]
            },
        }
    }

    fn target(&self, cx: &App) -> Option<RbacAccessReviewTarget> {
        let text = |input: &Entity<InputState>| input.read(cx).value().trim().to_owned();
        let optional = |input: &Entity<InputState>| Some(text(input)).filter(|v| !v.is_empty());
        let verb = text(&self.verb);
        if verb.is_empty() {
            return None;
        }
        if self.non_resource {
            let url = text(&self.url);
            return url
                .starts_with('/')
                .then_some(RbacAccessReviewTarget::NonResource {
                    verb,
                    non_resource_url: url,
                });
        }
        let resource = text(&self.resource);
        (!resource.is_empty()).then(|| RbacAccessReviewTarget::Resource {
            verb,
            resource,
            api_group: text(&self.api_group),
            namespace: optional(&self.target_namespace),
            subresource: optional(&self.subresource),
            name: optional(&self.resource_name),
        })
    }
}

fn field(input: &Entity<InputState>, id: &'static str, label: &'static str) -> Input {
    Input::new(input).id(id).aria_label(label).text_xs().h_7()
}

impl Inspector {
    /// `beginIdentity`.
    pub(super) fn begin_identity(
        &mut self,
        kind: IdentityKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let verifier = &mut self.rbac.verifier;
        verifier.manual_kind = kind;
        let groups = identity_defaults(kind, "").join(", ");
        verifier
            .name
            .update(cx, |input, cx| input.set_value("", window, cx));
        verifier
            .namespace
            .update(cx, |input, cx| input.set_value("default", window, cx));
        verifier
            .groups
            .update(cx, |input, cx| input.set_value(groups, window, cx));
        self.rbac.identity = IdentitySource::Manual;
        self.rbac.verifier.reset();
        cx.notify();
    }

    /// `verify()`: one SubjectAccessReview for the active identity and target.
    fn verify_rbac(&mut self, cx: &mut Context<Self>) {
        let identity = self.rbac.active_identity(cx);
        let state = &mut self.rbac;
        let (Some(data), Some(target)) = (state.data(), state.verifier.target(cx)) else {
            return;
        };
        if identity.name.is_empty() || matches!(state.verifier.review, Review::Checking(_)) {
            return;
        }
        let request = RbacAccessReviewRequest {
            cluster_context: data.cluster.clone(),
            identity: identity.review_identity(),
            target,
            kubeconfig_env_var: None,
        };
        let submitted = Submitted {
            timestamp: chrono::Local::now()
                .format("%d/%m/%Y, %H:%M:%S")
                .to_string(),
            identity: serde_json::to_string(&request.identity).unwrap_or_default(),
            target: serde_json::to_string(&request.target).unwrap_or_default(),
        };
        let verifier = &mut state.verifier;
        verifier.review = Review::Checking(submitted);
        verifier.request = Rc::new(());
        let token = verifier.request.clone();
        let read = self.backend.review_rbac(request);
        let read = self.timings.measure("review_rbac", read);
        self.rbac.verifier.task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                let verifier = &mut this.rbac.verifier;
                if !Rc::ptr_eq(&verifier.request, &token) {
                    return;
                }
                let Review::Checking(submitted) =
                    std::mem::replace(&mut verifier.review, Review::None)
                else {
                    return;
                };
                verifier.review = match result {
                    Ok(result) => Review::Done(submitted, result),
                    Err(error) => Review::Failed(submitted, error.message),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn rbac_verifier(&self, identity: &Identity, cx: &mut Context<Self>) -> Div {
        let state = &self.rbac;
        let verifier = &state.verifier;
        let manual = matches!(state.identity, IdentitySource::Manual) || identity.name.is_empty();
        let checking = matches!(verifier.review, Review::Checking(_));
        let ready = !checking && !identity.name.is_empty() && verifier.target(cx).is_some();
        let toggle = |id: &'static str, label: &'static str, on: bool| {
            button(
                if on {
                    Variant::Default
                } else {
                    Variant::Outline
                },
                20.,
                false,
            )
            .id(id)
            .px(px(6.))
            .child(label)
        };
        let identity_block = if manual {
            let kind = verifier.manual_kind;
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(div().flex().flex_wrap().gap(px(4.)).children(
                    IdentityKind::ALL.into_iter().map(|(option, label)| {
                        toggle(
                            match option {
                                IdentityKind::ServiceAccount => "rbac-identity-sa",
                                IdentityKind::User => "rbac-identity-user",
                                IdentityKind::Group => "rbac-identity-group",
                            },
                            label,
                            option == kind,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.begin_identity(option, window, cx),
                        ))
                    }),
                ))
                .child(field(&verifier.name, "rbac-identity-name", "Identity name"))
                .when(kind == IdentityKind::ServiceAccount, |block| {
                    block.child(field(
                        &verifier.namespace,
                        "rbac-identity-namespace",
                        "ServiceAccount namespace",
                    ))
                })
                .when(kind == IdentityKind::User, |block| {
                    block.child(field(&verifier.groups, "rbac-identity-groups", "Groups"))
                })
        } else {
            let label = identity.label();
            let kind = identity.kind;
            div()
                .p(px(8.))
                .rounded(px(4.))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::muted().opacity(0.25))
                .child(caps("Identity"))
                .child(
                    ui::medium(ui::xs(label.clone()))
                        .id("rbac-identity")
                        .test_support()
                        .aria_label(label)
                        .mt(px(4.)),
                )
                .child(
                    toggle("rbac-change-identity", "Change identity", false)
                        .mt(px(8.))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.begin_identity(kind, window, cx)
                        })),
                )
        };
        let non_resource = verifier.non_resource;
        let inputs = if non_resource {
            div()
                .child(field(&verifier.verb, "rbac-verb", "Verb"))
                .child(field(&verifier.url, "rbac-url", "Non-resource URL"))
        } else {
            div()
                .child(field(&verifier.verb, "rbac-verb", "Verb"))
                .child(field(&verifier.resource, "rbac-resource", "Resource"))
                .child(field(
                    &verifier.api_group,
                    "rbac-api-group",
                    "API group, blank for core",
                ))
                .child(field(
                    &verifier.target_namespace,
                    "rbac-namespace",
                    "Namespace, blank for cluster",
                ))
                .child(field(
                    &verifier.subresource,
                    "rbac-subresource",
                    "Subresource, optional",
                ))
                .child(field(
                    &verifier.resource_name,
                    "rbac-resource-name",
                    "Resource name, optional",
                ))
        };
        ui::card()
            .child(
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(Tokens::border())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(ui::icon("shield-check", 16., Tokens::primary()))
                            .child(ui::semibold(ui::sm("Verify exact action"))),
                    )
                    .child(ui::muted(ui::xs("Live SubjectAccessReview. Manual only.")).mt(px(4.))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .p(px(12.))
                    .child(identity_block)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(
                                toggle("rbac-target-resource", "Resource", !non_resource).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.rbac.verifier.non_resource = false;
                                        this.rbac.verifier.reset();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                toggle("rbac-target-non-resource", "Non-resource", non_resource)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.rbac.verifier.non_resource = true;
                                        this.rbac.verifier.reset();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(inputs.flex().flex_col().gap(px(8.)))
                    .child(
                        button(Variant::Default, 24., !ready)
                            .id("rbac-verify")
                            .test_support()
                            .aria_label("Verify access")
                            .w_full()
                            .child(ui::icon("shield-check", 12., Tokens::primary_foreground()))
                            .child(if checking {
                                "Verifying…"
                            } else {
                                "Verify access"
                            })
                            .when(ready, |button| {
                                button.on_click(cx.listener(|this, _, _, cx| this.verify_rbac(cx)))
                            }),
                    )
                    .child(ui::muted(ui::text(
                        "Result is never inferred from observed grants.",
                        10.,
                        14.,
                    ))),
            )
    }

    pub(super) fn rbac_latest(&self, data: &RbacInspectionSummary) -> Option<AnyElement> {
        let (submitted, result, failure) = match &self.rbac.verifier.review {
            Review::None => return None,
            Review::Checking(submitted) => (submitted, None, None),
            Review::Done(submitted, result) => (submitted, Some(result), None),
            Review::Failed(submitted, message) => (submitted, None, Some(message)),
        };
        let outcome = result.map(|result| match result.outcome {
            RbacAccessReviewOutcome::Allowed => ("allowed", "Allowed", Palette::emerald300()),
            RbacAccessReviewOutcome::Denied => ("denied", "Denied", Tokens::destructive()),
            RbacAccessReviewOutcome::NoOpinion => ("noOpinion", "No opinion", Palette::amber300()),
        });
        Some(
            div()
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(Tokens::border())
                .child(caps("Latest verification"))
                .when_some(outcome, |panel, (value, label, tone)| {
                    panel.child(
                        ui::semibold(ui::sm(label))
                            .id("rbac-verdict")
                            .test_support()
                            .aria_label(value)
                            .mt(px(8.))
                            .text_color(tone),
                    )
                })
                .when_some(
                    result.and_then(|result| result.reason.clone()),
                    |panel, reason| panel.child(ui::muted(ui::xs(reason)).mt(px(4.))),
                )
                .when_some(
                    result.and_then(|result| result.evaluation_error.clone()),
                    |panel, error| {
                        panel.child(ui::xs(error).mt(px(4.)).text_color(Tokens::destructive()))
                    },
                )
                .when_some(failure.cloned(), |panel, message| {
                    panel.child(
                        ui::xs(format!("Verifier unavailable · {message}"))
                            .mt(px(8.))
                            .text_color(Tokens::destructive()),
                    )
                })
                .child(
                    ui::muted(ui::text(
                        format!(
                            "{} · {}\n{}\n{}",
                            data.cluster, submitted.timestamp, submitted.identity, submitted.target
                        ),
                        10.,
                        14.,
                    ))
                    .mt(px(8.)),
                )
                .into_any_element(),
        )
    }
}
