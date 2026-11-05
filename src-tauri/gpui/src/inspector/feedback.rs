//! Common copy announcements, technical errors and exact permission handoffs.
use super::{
    Inspector, Page,
    detail::{Detail, DetailTab},
};
use crate::{backend::Scope, theme::Tokens, ui};
use gpui_kit::{
    AnyElement, ClipboardItem, Context, SharedString, Task, TestSupportExt, Window, prelude::*, px,
};
use kubecove_lib::models::{RbacAccessReviewTarget, ResourceSummary};

#[derive(Default)]
pub(super) struct Feedback {
    expanded: Option<(String, String)>,
    announcement: Option<String>,
    copy_task: Option<Task<()>>,
    origin: Option<Origin>,
}

struct Origin {
    page: Page,
    scope: Scope,
    workspace: Option<u64>,
    resource: Option<ResourceSummary>,
    tab: Option<DetailTab>,
}

impl Inspector {
    /// GPUI's clipboard adapter has no fallible write result. Fallible adapters
    /// can use `announce_copy` with their actual result instead.
    pub(super) fn copy_text(&mut self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.announce_copy(Ok(()), cx);
    }

    pub(super) fn announce_copy(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.feedback.announcement = Some(match result {
            Ok(()) => "Copied".into(),
            Err(_) => "Copy failed".into(),
        });
        // FriendlyError.svelte resets its copied label after 1200 ms.
        self.feedback.copy_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1200))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.feedback.announcement = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn copy_announcement(&self) -> Option<AnyElement> {
        self.feedback.announcement.as_ref().map(|text| {
            ui::panel()
                .id("copy-announcement")
                .test_support()
                .role(gpui_kit::Role::Status)
                .aria_label(text.clone())
                .absolute()
                .bottom(px(16.))
                .right(px(16.))
                .p(px(12.))
                .child(ui::xs(text.clone()))
                .into_any_element()
        })
    }

    /// Callers supply the exact target. Error text never invents a permission.
    pub(super) fn friendly_failure(
        &self,
        id: &'static str,
        title: &str,
        detail: &str,
        permission: Option<(String, RbacAccessReviewTarget)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let expanded = self
            .feedback
            .expanded
            .as_ref()
            .is_some_and(|(key, value)| key == id && value == detail);
        let detail = detail.to_owned();
        let toggle_detail = detail.clone();
        let copy_detail = detail.clone();
        ui::panel()
            .id(id)
            .test_support()
            .role(gpui_kit::Role::Alert)
            .aria_label(title.to_owned())
            .p(px(12.))
            .border_color(Tokens::destructive().opacity(0.4))
            .child(ui::semibold(ui::sm(title.to_owned())))
            .child(
                ui::muted(ui::xs(
                    "The original technical error is still available below.",
                ))
                .mt(px(8.)),
            )
            .child(ui::muted(ui::xs("Copy the detail if this needs debugging.")).mt(px(8.)))
            .when_some(permission, |panel, (context, target)| {
                panel.child(
                    ui::outline_button()
                        .id(SharedString::from(format!("{id}:permission")))
                        .test_support()
                        .child("Inspect required permission")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.inspect_permission(context.clone(), target.clone(), window, cx);
                        })),
                )
            })
            .child(
                ui::outline_button()
                    .id(SharedString::from(format!("{id}:detail")))
                    .test_support()
                    .aria_label("Technical detail")
                    .child("Technical detail")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.feedback.expanded = if expanded {
                            None
                        } else {
                            Some((id.into(), toggle_detail.clone()))
                        };
                        cx.notify();
                    })),
            )
            .when(expanded, |panel| {
                panel.child(ui::xs(detail).mt(px(8.))).child(
                    ui::outline_button()
                        .id(SharedString::from(format!("{id}:copy")))
                        .test_support()
                        .aria_label("Copy detail")
                        .child("Copy detail")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.copy_text(copy_detail.clone(), cx)
                        })),
                )
            })
            .into_any_element()
    }

    pub(super) fn inspect_permission(
        &mut self,
        context: String,
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
        };
        self.open_rbac(None, window, cx);
        if self.page != Page::Rbac {
            return;
        }
        if self.scope.context != context {
            self.scope = Scope {
                context,
                namespace: None,
            };
            self.load_scope(false, window, cx);
        }
        self.feedback.origin = Some(origin);
        self.fill_permission_target(target, window, cx);
        cx.notify();
    }

    pub(super) fn permission_return(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.feedback.origin.as_ref().map(|_| {
            ui::outline_button()
                .id("permission-return")
                .test_support()
                .aria_label("Return to source")
                .child("Return to source")
                .on_click(cx.listener(|this, _, window, cx| {
                    let Some(origin) = this.feedback.origin.take() else {
                        return;
                    };
                    if origin.workspace != this.launcher.active {
                        return;
                    }
                    this.scope = origin.scope;
                    this.open_page(origin.page, cx);
                    this.load_scope(false, window, cx);
                    if let Some(resource) = origin.resource {
                        this.open_resource_on(resource, origin.tab, window, cx);
                    }
                }))
                .into_any_element()
        })
    }
}
