//! Which topology nodes join the map as links of a narrowed kind selection,
//! as `topologyLinkKinds` and `filterTopologyByTableRows` decide on the web.

use super::super::Inspector;
use kubecove_lib::models::ResourceTopology;

impl Inspector {
    /// Per topology node, whether its kind is outside the kind selection, when
    /// the selection leaves out a workspace default kind. The map then also
    /// keeps nodes one edge from a matching resource, so its edges still read.
    /// `None` keeps the map to matching resources and their owners.
    pub(super) fn map_links(&self, topology: &ResourceTopology) -> Option<Vec<bool>> {
        let narrowed = self.live.selected.is_some()
            || self.palette.kind.is_some()
            || self
                .base_kinds()
                .iter()
                .any(|kind| !self.kind_selected(kind));
        narrowed.then(|| {
            topology
                .nodes
                .iter()
                .map(|node| {
                    let summary = &node.summary;
                    let selected = self.live.selected.as_ref().is_none_or(|kind| {
                        summary.kind == kind.kind
                            && summary.api_version.as_ref() == Some(&kind.api_version)
                    }) && self.palette.kind.is_none_or(|kind| summary.kind == kind)
                        && self.selected_kind(&summary.kind, summary.api_version.as_deref());
                    !selected
                })
                .collect()
        })
    }
}
