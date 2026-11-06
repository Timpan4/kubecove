//! A reactive boundary for navigation whose closed branches have unchanged inputs.
use super::{Inspector, Page};
use gpui_kit::{
    AnyElement, AnyView, AppContext, Context, Entity, FocusHandle, Pixels, Point, Render,
    SharedString, StyleRefinement, WeakEntity, Window, div, prelude::*, px,
};

#[derive(Clone, PartialEq)]
struct Inputs {
    workspace_name: SharedString,
    context: String,
    namespace: Option<String>,
    namespaces: Vec<String>,
    selected: Option<String>,
    application_workspace: bool,
    page: Page,
    gitops_location: &'static str,
    rbac_kind_location: bool,
    focus: Option<FocusHandle>,
    scroll: Point<Pixels>,
    width: f32,
}

pub(super) struct SidebarCache {
    owner: WeakEntity<Inspector>,
    inputs: Option<Inputs>,
}

impl Render for SidebarCache {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _span = crate::frame_profile::scope("sidebar", "child_render");
        let Some(inputs) = &self.inputs else {
            return div().into_any_element();
        };
        let width = inputs.width;
        self.owner
            .update(cx, |owner, cx| {
                owner.sidebar_contents(window, cx, width).into_any_element()
            })
            .unwrap_or_else(|_| div().into_any_element())
    }
}

impl Inspector {
    pub(super) fn sidebar(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
        width: f32,
    ) -> AnyElement {
        if !self.sidebar_tree.open.is_empty()
            || self.live.custom_open
            || !self.gitops_page.nav_open.is_empty()
            || self.sidebar_tree.reveal.borrow().is_some()
        {
            if let Some(cache) = self.sidebar_tree.cache.borrow().as_ref()
                && cache.read(cx).inputs.is_some()
            {
                cache.update(cx, |cache, _| cache.inputs = None);
            }
            return self.sidebar_contents(window, cx, width).into_any_element();
        }
        let _span = crate::frame_profile::scope("sidebar", "cache_inputs");
        let inputs = Inputs {
            workspace_name: self.workspace_name.clone(),
            context: self.scope.context.clone(),
            namespace: self.scope.namespace.clone(),
            namespaces: self
                .namespaces
                .iter()
                .filter(|namespace| self.launcher.shows_namespace(&namespace.name))
                .map(|namespace| namespace.name.clone())
                .collect(),
            selected: self.sidebar_tree.selected.clone(),
            application_workspace: self.application_workspace_name().is_some(),
            page: self.page,
            gitops_location: self.gitops_location(),
            rbac_kind_location: self.rbac.kind_location,
            focus: self.sidebar_tree.focused(window, cx),
            scroll: self.sidebar_tree.scroll.offset(),
            width,
        };
        let owner = cx.entity().downgrade();
        let cache: Entity<SidebarCache> = self
            .sidebar_tree
            .cache
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|_| SidebarCache {
                    owner,
                    inputs: Some(inputs.clone()),
                })
            })
            .clone();
        let changed = cache.read(cx).inputs.as_ref() != Some(&inputs);
        if changed {
            let _span = crate::frame_profile::scope("sidebar", "inputs_changed");
            cache.update(cx, |cache, cx| {
                cache.inputs = Some(inputs);
                cx.notify();
            });
        }
        // Changed labels and selection render in this frame without recycling old paint.
        let view = if changed {
            cache.into_any_element()
        } else {
            AnyView::from(cache)
                .cached(StyleRefinement::default().size_full())
                .into_any_element()
        };
        div()
            .w(px(width))
            .h_full()
            .flex_none()
            .child(view)
            .into_any_element()
    }
}
