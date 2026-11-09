//! Semantic editing commands for the focused widget.

/// An editing command for the focused widget in a focused window.
///
/// Commands follow the same message and clipboard paths as keyboard editing,
/// independently of the widget's keyboard bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Copy the current selection to the clipboard.
    Copy,
    /// Copy the current selection to the clipboard and remove it.
    Cut,
    /// Paste text from the clipboard.
    Paste,
    /// Select all the text in the focused widget.
    SelectAll,
}
