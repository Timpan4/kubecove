//! Shared failure feedback: the `FriendlyError.svelte` panel (classified copy,
//! collapsible technical detail, Copy detail and the exact RBAC handoff) and
//! the `CopyToast.svelte` notification that `copy_toast` renders.
use super::{
    Inspector, Page,
    detail::{Detail, DetailTab},
    friendly_error::{self, Bucket, Context as ErrorContext, Presentation},
};
use crate::targets::TestSupportExt;
use crate::{
    backend::Scope,
    theme::{Palette, Tokens},
    ui,
};
use gpui_kit::{AnyElement, ClipboardItem, Context, SharedString, Task, Window, prelude::*, px};
use kubecove_lib::models::{AppError, AppErrorKind, RbacAccessReviewTarget, ResourceSummary};
use std::{collections::HashSet, time::Duration};

#[derive(Default)]
pub(super) struct Feedback {
    /// Error panels whose technical detail is open.
    expanded: HashSet<&'static str>,
    /// The panel whose Copy detail button shows the last result, and whether
    /// the copy succeeded.
    copy: Option<(&'static str, bool)>,
    copy_task: Option<Task<()>>,
    /// The persistent copy notification. It stays until dismissed or replaced.
    toast: Option<String>,
    origin: Option<Origin>,
}

/// The RBAC handoff a failed read can offer.
pub(super) struct Permission {
    pub cluster: String,
    pub target: RbacAccessReviewTarget,
}

/// Technical-detail and copy state for every error panel, copied out of
/// `Feedback` so renderers that cannot reach the inspector take it as input.
#[derive(Clone, Default)]
pub(super) struct Disclosure {
    expanded: HashSet<&'static str>,
    copied: Option<(&'static str, bool)>,
}

impl Feedback {
    pub(super) fn disclosure(&self) -> Disclosure {
        Disclosure {
            expanded: self.expanded.clone(),
            copied: self.copy,
        }
    }
}

struct Origin {
    page: Page,
    scope: Scope,
    workspace: Option<u64>,
    resource: Option<ResourceSummary>,
    tab: Option<DetailTab>,
    source: String,
}

impl Inspector {
    pub(super) fn page_failure(
        &self,
        title: &'static str,
        message: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.friendly_failure(
            title,
            None,
            &message,
            ErrorContext {
                fallback_title: Some(title),
                ..Default::default()
            },
            None,
            cx,
        )
    }

    /// A failed page read keeps its typed kind, so the panel classifies it as
    /// `FriendlyError.svelte` does instead of by message text alone.
    pub(super) fn page_read_failure(
        &self,
        title: &'static str,
        error: &AppError,
        operation: friendly_error::Operation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.friendly_failure(
            title,
            Some(error.kind),
            &error.message,
            ErrorContext {
                operation: Some(operation),
                fallback_title: Some(title),
                ..Default::default()
            },
            None,
            cx,
        )
    }

    /// A partial read failure in compact mode, as the web reports it beside
    /// data that still loads. The fallback title replaces the classified one.
    pub(super) fn partial_read_failure(
        &self,
        title: &'static str,
        error: &AppError,
        operation: friendly_error::Operation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.friendly_failure(
            title,
            Some(error.kind),
            &error.message,
            ErrorContext {
                operation: Some(operation),
                fallback_title: Some(title),
                partial: true,
                compact: true,
                ..Default::default()
            },
            None,
            cx,
        )
    }

    /// Writes the detail and shows the panel's own result for 1200 ms, as
    /// `FriendlyError.svelte` does. GPUI's clipboard write cannot fail, so
    /// the failed label is reached only through `finish_detail_copy`.
    pub(super) fn copy_detail(&mut self, id: &'static str, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.finish_detail_copy(id, Ok(()), cx);
    }

    fn finish_detail_copy(
        &mut self,
        id: &'static str,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        self.feedback.copy = Some((id, result.is_ok()));
        self.feedback.copy_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.feedback.copy = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn toggle_technical_detail(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if !self.feedback.expanded.remove(id) {
            self.feedback.expanded.insert(id);
        }
        cx.notify();
    }

    /// `CopyToast.svelte`'s message. `CopyableText` sets it after each copy.
    pub(super) fn show_copy_toast(&mut self, message: String, cx: &mut Context<Self>) {
        self.feedback.toast = Some(message);
        cx.notify();
    }

    pub(super) fn copy_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.feedback.toast.as_ref().map(|text| {
            ui::panel()
                .id("copy-toast")
                .test_support()
                .role(gpui_kit::Role::Status)
                .aria_label(text.clone())
                .absolute()
                .bottom(px(16.))
                .right(px(16.))
                .max_w(px(384.))
                .p(px(12.))
                .flex()
                .items_start()
                .gap(px(12.))
                .child(ui::xs(text.clone()).min_w_0())
                .child(
                    ui::outline_button()
                        .id("copy-toast-dismiss")
                        .test_support()
                        .aria_label("Dismiss copy notification")
                        .child(ui::icon("x", 14., Tokens::muted_foreground()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.feedback.toast = None;
                            cx.notify();
                        })),
                )
                .into_any_element()
        })
    }

    /// The shared error panel. `kind` and `message` come from the failed read;
    /// `permission` is offered only when the classified error is a forbidden
    /// RBAC denial, as `FriendlyError.svelte` does.
    pub(super) fn friendly_failure(
        &self,
        id: &'static str,
        kind: Option<AppErrorKind>,
        message: &str,
        context: ErrorContext<'_>,
        permission: Option<Permission>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        Self::failure_panel(
            id,
            kind,
            message,
            context,
            permission,
            &self.feedback.disclosure(),
            cx,
        )
    }

    /// Records the Argo CD snapshot failure's presentation, using the web's
    /// transport-specific title as the fallback for unclassified errors.
    pub(super) fn argo_snapshot_failure(
        error: &AppError,
        connected: bool,
        disclosure: &Disclosure,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = if connected {
            "Connected Argo CD snapshot unavailable"
        } else {
            "Kubernetes Application snapshot unavailable"
        };
        Self::failure_panel(
            "argo-snapshot-error",
            Some(error.kind),
            &error.message,
            ErrorContext {
                operation: Some(friendly_error::Operation::Details),
                fallback_title: Some(title),
                ..Default::default()
            },
            None,
            disclosure,
            cx,
        )
    }

    fn failure_panel(
        id: &'static str,
        kind: Option<AppErrorKind>,
        message: &str,
        context: ErrorContext<'_>,
        permission: Option<Permission>,
        disclosure: &Disclosure,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let presentation = friendly_error::present(kind, message, &context);
        // `FriendlyError.svelte` compact mode hides the next step and the
        // "still works" note, but keeps the summary and technical detail.
        let compact = context.compact;
        let permission = permission.filter(|_| presentation.bucket == Bucket::ForbiddenRbac);
        let expanded = disclosure.expanded.contains(id);
        let copied = disclosure
            .copied
            .filter(|(key, _)| *key == id)
            .map(|(_, ok)| ok);
        // Shown, read aloud and copied, so credentials are removed as the web's
        // `messageFromFriendlyError` does.
        let technical = friendly_error::redact_detail(message);
        let copy_text = technical.clone();
        let description = description(&presentation, compact);
        let border = if presentation.warning {
            Palette::amber500(0.35)
        } else {
            Tokens::destructive().opacity(0.4)
        };
        ui::panel()
            .id(id)
            .test_support()
            .role(gpui_kit::Role::Alert)
            .aria_label(presentation.title.clone())
            .aria_description(description)
            .px(px(12.))
            .py(px(if compact { 8. } else { 12. }))
            .border_color(border)
            .child(ui::semibold(ui::sm(presentation.title.clone())))
            .child(ui::muted(ui::xs(presentation.summary.clone())).mt(px(8.)))
            .when_some(
                presentation.still_works.filter(|_| !compact),
                |panel, text| panel.child(ui::muted(ui::xs(text)).mt(px(8.))),
            )
            .when_some(presentation.next.filter(|_| !compact), |panel, text| {
                panel.child(ui::muted(ui::xs(text)).mt(px(8.)))
            })
            .when_some(permission, |panel, permission| {
                panel.child(
                    ui::outline_button()
                        .id(SharedString::from(format!("{id}:permission")))
                        .test_support()
                        .child("Inspect required permission")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.inspect_permission(
                                permission.cluster.clone(),
                                permission.target.clone(),
                                window,
                                cx,
                            );
                        })),
                )
            })
            .child(
                ui::outline_button()
                    .id(SharedString::from(format!("{id}:detail")))
                    .test_support()
                    .aria_label("Technical detail")
                    .child("Technical detail")
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_technical_detail(id, cx)),
                    ),
            )
            .when(expanded, |panel| {
                panel
                    .child(
                        ui::xs(technical.clone())
                            .id(SharedString::from(format!("{id}:technical")))
                            .test_support()
                            .aria_label(technical)
                            .mt(px(8.)),
                    )
                    .child(
                        ui::outline_button()
                            .id(SharedString::from(format!("{id}:copy")))
                            .test_support()
                            .aria_label(if copied == Some(true) {
                                "Copied"
                            } else {
                                "Copy detail"
                            })
                            .child(if copied == Some(true) {
                                "Copied"
                            } else {
                                "Copy detail"
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.copy_detail(id, copy_text.clone(), cx)
                            })),
                    )
                    .when(copied == Some(false), |row| {
                        row.child(ui::muted(ui::xs("Copy failed")))
                    })
            })
            .into_any_element()
    }

    /// Hands the exact target to the RBAC verifier and records where to return.
    pub(super) fn inspect_permission(
        &mut self,
        cluster: String,
        target: RbacAccessReviewTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let origin = Origin {
            page: self.page,
            scope: self.scope.clone(),
            workspace: self.launcher.active,
            resource: self.detail.resource().cloned(),
            tab: match &self.detail {
                Detail::Ready(detail) => Some(detail.tab),
                _ => None,
            },
            source: self.shell_title(),
        };
        self.open_rbac(None, window, cx);
        if self.page != Page::Rbac {
            return;
        }
        if self.scope.context != cluster {
            self.scope = Scope {
                context: cluster,
                namespace: None,
            };
            self.load_scope(false, window, cx);
        }
        self.feedback.origin = Some(origin);
        self.fill_permission_target(target, window, cx);
        cx.notify();
    }

    pub(super) fn permission_return(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.feedback.origin.as_ref().map(|origin| {
            ui::outline_button()
                .id("permission-return")
                .test_support()
                .aria_label(format!("Return to {}", origin.source))
                .child(format!("Return to {}", origin.source))
                .on_click(cx.listener(|this, _, window, cx| {
                    let Some(origin) = this.feedback.origin.take() else {
                        return;
                    };
                    if origin.workspace != this.launcher.active {
                        return;
                    }
                    let previous = std::mem::replace(&mut this.scope, origin.scope.clone());
                    this.open_page(origin.page, cx);
                    if this.page != origin.page {
                        // A draft or exec session refused or deferred the navigation:
                        // keep the current scope and the return target for another try.
                        this.scope = previous;
                        this.feedback.origin = Some(origin);
                        cx.notify();
                        return;
                    }
                    this.load_scope(false, window, cx);
                    if let Some(resource) = origin.resource {
                        this.open_resource_on(resource, origin.tab, window, cx);
                    }
                }))
                .into_any_element()
        })
    }
}

/// The visible text, which `aria_description` carries so drivers can read it.
fn description(presentation: &Presentation, compact: bool) -> String {
    let visible = |text: Option<&'static str>| text.filter(|_| !compact);
    [
        Some(presentation.summary.as_str()),
        visible(presentation.still_works),
        visible(presentation.next),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
}
