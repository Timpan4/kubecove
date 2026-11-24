//! The YAML tab of the open resource: the editor's folds, indent tints and
//! inline diagnostics, the shape and encoding pickers, the Kubernetes lint,
//! and the guarded apply flow (ADR 0004). The rest of the inspector uses the
//! names exported here and the `Inspector` methods the family marks
//! `pub(in crate::inspector)`.

mod apply;
mod folds;
mod lens;
mod lint;
mod marks;
mod parse;
mod tab;
mod tags;
mod view;

pub(super) use apply::YamlApply;
pub(super) use folds::factory as fold_factory;
pub(super) use lens::Lens;
pub(super) use lint::Lint;
pub(super) use marks::YamlMarks;
pub(super) use tab::yaml_tab;
pub(super) use view::YamlView;

use gpui_kit::component::input::EditorState;
use gpui_kit::{Context, SharedString, Window};

/// A YAML editor over `text` with line numbers, which every YAML document view
/// starts from. The detail panel's editor adds its folds and indent tints.
pub(super) fn viewer(
    text: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut Context<EditorState>,
) -> EditorState {
    EditorState::new(window, cx)
        .language("yaml")
        .line_number(true)
        .default_value(text)
}
