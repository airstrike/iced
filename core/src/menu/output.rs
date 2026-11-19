use crate::edit;

/// The result of activating an item in a menu dispatch.
#[derive(Debug)]
pub enum Output<Message> {
    /// A freshly produced application message.
    Message(Message),
    /// A semantic editing command, unaffected by message mapping.
    Edit(edit::Command),
}
