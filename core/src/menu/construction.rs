use super::{Bar, Entry, Item, Menu};
use crate::edit;

/// Creates a native menu bar from the given menus.
pub fn bar<Message>(menus: impl IntoIterator<Item = Menu<Message>>) -> Bar<Message> {
    Bar::new(menus)
}

/// Creates a named menu from an iterator of items, submenus, or entries.
///
/// For a list of mixed entry types, use [`menu!`](crate::menu!).
///
/// ```
/// use iced_core::menu;
///
/// let recent = menu("Recent", (1..=3).map(|number| {
///     menu::item(format!("Document {number}")).on_press(number)
/// }));
/// assert_eq!(recent.entries().len(), 3);
/// ```
pub fn menu<Message>(
    title: impl Into<String>,
    entries: impl IntoIterator<Item = impl Into<Entry<Message>>>,
) -> Menu<Message> {
    Menu::with_entries(title, entries)
}

/// Creates a named menu, converting each child into an [`Entry`].
///
/// Items, separators, and nested menus can be mixed without explicit `.into()`
/// calls. Use [`menu()`](fn@crate::menu) for an iterator of entries.
///
/// ```
/// use iced_core::menu;
///
/// let file = menu![
///     "File",
///     menu::item("New").on_press(1),
///     menu::separator(),
///     menu!["Recent", menu::item("Document").on_press(2)],
/// ];
/// assert_eq!(file.entries().len(), 3);
///
/// let empty: menu::Menu<()> = menu!["Empty"];
/// assert!(empty.entries().is_empty());
/// ```
#[macro_export]
macro_rules! menu {
    ($title:expr $(,)?) => (
        $crate::menu::Menu::new($title)
    );
    ($title:expr, $($entry:expr),+ $(,)?) => (
        $crate::menu::Menu::with_entries(
            $title,
            [$(::core::convert::Into::<$crate::menu::Entry<_>>::into($entry)),+],
        )
    );
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
    crate::menu![
        "Edit",
        Item::edit(edit::Command::Cut),
        Item::edit(edit::Command::Copy),
        Item::edit(edit::Command::Paste),
        separator(),
        Item::edit(edit::Command::SelectAll),
    ]
}
