//! `ForegroundLoadingBar.svelte`: a 2 px bar across the top while a foreground
//! read runs. The web counts `withForegroundLoad` calls. Native derives the
//! same state from the resource list, topology and detail reads it owns.
use super::{Inspector, Load, Page, detail::Detail, map::TopologyLoad};
use crate::targets::TestSupportExt;
use crate::theme::Tokens;
use gpui_kit::{IntoElement, div, prelude::*, px};

impl Inspector {
    fn foreground_loading(&self) -> bool {
        let list = matches!(self.page, Page::Resources | Page::ClusterOverview)
            && matches!(*self.observation.status(), Load::Loading);
        let topology = matches!(self.map.topology, TopologyLoad::Loading)
            && matches!(self.page, Page::Resources | Page::ClusterOverview);
        let detail = matches!(self.detail, Detail::Loading(_));
        list || topology || detail
    }

    /// Absolutely positioned over the shell's top padding, so the layout does
    /// not move when the bar appears.
    pub(super) fn foreground_bar(&self) -> impl IntoElement {
        let active = self
            .loading
            .phase("foreground", self.foreground_loading())
            .shown();
        div()
            .id("foreground-loading-bar")
            .test_support()
            .aria_description(if active { "active" } else { "idle" })
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(2.))
            .overflow_hidden()
            .when(active, |bar| {
                bar.child(div().h_full().w_1_3().rounded_full().bg(Tokens::primary()))
            })
    }
}
