use super::{Item, Menu, description, dispatch};

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
    pub(super) fn into_parts(self) -> (description::Entry, dispatch::Entry<Message>) {
        match self {
            Self::Item(item) => {
                let (description, action) = item.into_parts();
                (
                    description::Entry::Item(description),
                    dispatch::Entry::Item(action),
                )
            }
            Self::Submenu(menu) => {
                let (description, dispatch) = menu.into_parts();
                (
                    description::Entry::Submenu(description),
                    dispatch::Entry::Menu(dispatch),
                )
            }
            Self::Separator => (description::Entry::Separator, dispatch::Entry::Separator),
        }
    }
}
