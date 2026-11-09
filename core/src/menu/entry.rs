use std::rc::Rc;

use super::{Item, Menu};

/// An entry in a native menu.
#[derive(Debug)]
pub enum Entry<Message> {
    /// An actionable menu item.
    Item(Item<Message>),
    /// A nested menu.
    Submenu(Menu<Message>),
    /// A visual separator between groups of entries.
    Separator,
}

impl<Message> From<Item<Message>> for Entry<Message> {
    fn from(item: Item<Message>) -> Self {
        Self::Item(item)
    }
}

impl<Message> From<Menu<Message>> for Entry<Message> {
    fn from(menu: Menu<Message>) -> Self {
        Self::Submenu(menu)
    }
}

impl<Message> Entry<Message> {
    pub(super) fn map_with<Other>(self, f: Rc<dyn Fn(Message) -> Other>) -> Entry<Other>
    where
        Message: 'static,
        Other: 'static,
    {
        match self {
            Self::Item(item) => Entry::Item(item.map(move |message| f(message))),
            Self::Submenu(menu) => Entry::Submenu(menu.map_with(f)),
            Self::Separator => Entry::Separator,
        }
    }
}
