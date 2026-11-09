use std::fmt;
use std::rc::Rc;

use crate::edit;

/// An action invoked when an enabled menu item is activated.
pub enum Action<Message> {
    /// Produces a fresh application message on each activation.
    Message(Rc<dyn Fn() -> Message>),
    /// Executes a semantic editing command in the focused widget.
    Edit(edit::Command),
}

impl<Message> Clone for Action<Message> {
    fn clone(&self) -> Self {
        match self {
            Self::Message(f) => Self::Message(Rc::clone(f)),
            Self::Edit(command) => Self::Edit(*command),
        }
    }
}

impl<Message> fmt::Debug for Action<Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(_) => f.write_str("Message(..)"),
            Self::Edit(command) => f.debug_tuple("Edit").field(command).finish(),
        }
    }
}
