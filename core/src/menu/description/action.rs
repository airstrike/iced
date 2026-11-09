use crate::edit;

/// The kind of action associated with an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Produces an application message when activated.
    Message,
    /// Executes a semantic editing command.
    Edit(edit::Command),
}
