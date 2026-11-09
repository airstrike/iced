use super::{Bar, Entry, Item, Menu};
use crate::edit;

/// Creates a native menu bar from the given menus.
pub fn bar<Message>(menus: impl IntoIterator<Item = Menu<Message>>) -> Bar<Message> {
    Bar::new(menus)
}

/// Creates an empty, enabled menu with the given title.
pub fn submenu<Message>(title: impl Into<String>) -> Menu<Message> {
    Menu::new(title)
}

/// Creates an item without an action.
pub fn item<Message>(title: impl Into<String>) -> Item<Message> {
    Item::new(title)
}

/// Creates a separator between groups of menu entries.
pub fn separator<Message>() -> Entry<Message> {
    Entry::Separator
}

/// Creates an Edit menu with Cut, Copy, Paste, and Select All commands.
pub fn edit<Message>() -> Menu<Message> {
    Menu::new("Edit")
        .push(Item::edit(edit::Command::Cut))
        .push(Item::edit(edit::Command::Copy))
        .push(Item::edit(edit::Command::Paste))
        .push(separator())
        .push(Item::edit(edit::Command::SelectAll))
}
