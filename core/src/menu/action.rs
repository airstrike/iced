use std::fmt;

use crate::edit;

/// An action invoked when an enabled menu item is activated.
pub enum Action<Message> {
    /// Produces a fresh application message on each activation.
    Message(Box<dyn Fn() -> Message>),
    /// Executes a semantic editing command in the focused widget.
    Edit(edit::Command),
}

impl<Message> fmt::Debug for Action<Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(_) => f.write_str("Message(..)"),
            Self::Edit(command) => f.debug_tuple("Edit").field(command).finish(),
        }
    }
}
