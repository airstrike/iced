//! Edit the focused widget of a window.
//!
//! Commands are ignored if the target window is no longer focused or open.
//! They preserve normal widget messages and clipboard behavior.
//!
//! ```no_run
//! use iced_runtime::core::window;
//! use iced_runtime::{Task, edit};
//!
//! fn copy_selection(window: window::Id) -> Task<()> {
//!     edit::copy(window)
//! }
//! ```
pub use crate::core::edit::Command;

use crate::core::{Event, window};
use crate::task;
use crate::{Action, Task};

/// Performs a semantic editing command in the focused widget of `window`.
///
/// The command is ignored if `window` is no longer focused or open. Editing
/// widgets publish their usual messages; this task produces no output itself.
pub fn perform<T>(window: window::Id, command: Command) -> Task<T> {
    task::effect(Action::Event {
        window,
        event: Event::Edit(command),
    })
}

/// Copies the selection in the focused widget of `window` to the clipboard.
///
/// If no editing widget handles the command, selected display text is copied.
pub fn copy<T>(window: window::Id) -> Task<T> {
    perform(window, Command::Copy)
}

/// Cuts the selection in the focused widget of `window` to the clipboard.
pub fn cut<T>(window: window::Id) -> Task<T> {
    perform(window, Command::Cut)
}

/// Pastes clipboard text into the focused widget of `window`.
pub fn paste<T>(window: window::Id) -> Task<T> {
    perform(window, Command::Paste)
}

/// Selects all text in the focused widget of `window`.
pub fn select_all<T>(window: window::Id) -> Task<T> {
    perform(window, Command::SelectAll)
}
