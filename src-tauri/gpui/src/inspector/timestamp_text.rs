//! `TimestampText.svelte`: the timestamp's full value is the hover text and
//! accessible description of the label it decorates.

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{AnyElement, IntoElement, SharedString, TestSupportExt, div, prelude::*};

/// Wraps `content` with a hover tooltip. Without tooltip text the content
/// renders as it is.
pub(super) fn with_tooltip(
    id: String,
    tooltip: Option<String>,
    content: impl IntoElement,
) -> AnyElement {
    let Some(tooltip) = tooltip.filter(|value| !value.is_empty()) else {
        return content.into_any_element();
    };
    div()
        .id(SharedString::from(id))
        .test_support()
        .aria_description(tooltip.clone())
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .child(content)
        .into_any_element()
}
