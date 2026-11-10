//! The Exec tab: the terminal sessions of the open resource, their buffer,
//! input, selection, resize and the prompt that guards leaving a running
//! session. The rest of the inspector uses the two types exported here and
//! the `Inspector` methods the family marks `pub(in crate::inspector)`.

mod buffer;
mod cursor_move;
mod custom;
mod drag;
mod input;
mod keys;
mod leave;
mod menu;
mod reflow;
mod resize;
mod selection;
mod state;
#[cfg(test)]
#[path = "../../tests/native/terminal_buffer.rs"]
mod terminal_buffer_tests;
mod view;

pub(super) use leave::LeaveIntent;
pub(super) use state::ExecState;
