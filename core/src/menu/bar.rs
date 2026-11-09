use std::rc::Rc;

use super::Menu;

/// The application menus contributed by a program.
#[derive(Debug)]
pub struct Bar<Message> {
    menus: Vec<Menu<Message>>,
}

impl<Message> Default for Bar<Message> {
    fn default() -> Self {
        Self { menus: Vec::new() }
    }
}

impl<Message> From<Menu<Message>> for Bar<Message> {
    fn from(menu: Menu<Message>) -> Self {
        Self::new([menu])
    }
}

impl<Message, const N: usize> From<[Menu<Message>; N]> for Bar<Message> {
    fn from(menus: [Menu<Message>; N]) -> Self {
        Self::new(menus)
    }
}

impl<Message> From<Vec<Menu<Message>>> for Bar<Message> {
    fn from(menus: Vec<Menu<Message>>) -> Self {
        Self::from_vec(menus)
    }
}

impl<Message> IntoIterator for Bar<Message> {
    type Item = Menu<Message>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.menus.into_iter()
    }
}

impl<Message> Bar<Message> {
    /// Creates a menu bar from the given menus, in display order.
    pub fn new(menus: impl IntoIterator<Item = Menu<Message>>) -> Self {
        Self {
            menus: menus.into_iter().collect(),
        }
    }

    /// Uses an already allocated vector as the menu bar's contents.
    pub fn from_vec(menus: Vec<Menu<Message>>) -> Self {
        Self { menus }
    }

    /// Appends a menu to the bar.
    pub fn push(mut self, menu: Menu<Message>) -> Self {
        self.menus.push(menu);
        self
    }

    /// Returns the menus in display order.
    pub fn menus(&self) -> &[Menu<Message>] {
        &self.menus
    }

    /// Returns whether the bar contributes no menus.
    pub fn is_empty(&self) -> bool {
        self.menus.is_empty()
    }

    /// Transforms emitted messages lazily, preserving semantic edit actions.
    pub fn map<Other>(self, f: impl Fn(Message) -> Other + 'static) -> Bar<Other>
    where
        Message: 'static,
        Other: 'static,
    {
        let f: Rc<dyn Fn(Message) -> Other> = Rc::new(f);

        Bar::new(self.menus.into_iter().map(|menu| {
            let f = Rc::clone(&f);
            menu.map_with(f)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit;
    use crate::keyboard::Modifiers;
    use crate::menu::{self, Action, Entry, Item};

    use std::cell::Cell;

    #[test]
    fn mapping_nested_nonclone_messages_is_lazy_and_preserves_presentation() {
        struct Message(Cell<usize>);
        struct Other(usize);

        let produced = Rc::new(Cell::new(0));
        let mapped = Rc::new(Cell::new(0));
        let bar = Bar::new([
            Menu::new("File").enabled(false).push(
                Menu::new("Nested")
                    .push(
                        Item::new("Fresh")
                            .checked(true)
                            .shortcut(Modifiers::LOGO, 'N')
                            .on_press_with({
                                let produced = Rc::clone(&produced);
                                move || {
                                    produced.set(produced.get() + 1);
                                    Message(Cell::new(produced.get()))
                                }
                            }),
                    )
                    .push(
                        Item::new("Unavailable")
                            .enabled(false)
                            .on_press_with(|| Message(Cell::new(0))),
                    )
                    .push(menu::separator())
                    .push(Item::new("No action")),
            ),
            menu::edit(),
        ]);
        let bar = bar.map({
            let mapped = Rc::clone(&mapped);
            move |message| {
                mapped.set(mapped.get() + 1);
                Other(message.0.get())
            }
        });
        assert_eq!(produced.get(), 0);
        assert_eq!(mapped.get(), 0);
        assert!(!bar.menus()[0].is_enabled());
        let Entry::Submenu(nested) = &bar.menus()[0].entries()[0] else {
            panic!("nested menu")
        };
        assert_eq!(nested.title(), "Nested");
        let Entry::Item(item) = &nested.entries()[0] else {
            panic!("message item")
        };
        assert_eq!(item.title(), "Fresh");
        assert!(item.is_enabled());
        assert!(item.is_checked());
        assert_eq!(
            item.shortcut_key().map(menu::Shortcut::key),
            Some(&crate::keyboard::Key::from('n'))
        );
        let Some(Action::Message(produce)) = item.action() else {
            panic!("message factory")
        };
        assert_eq!(produce().0, 1);
        assert_eq!(produce().0, 2);
        assert_eq!(mapped.get(), 2);
        let Entry::Item(disabled) = &nested.entries()[1] else {
            panic!("disabled item")
        };
        assert!(!disabled.is_enabled());
        assert!(matches!(nested.entries()[2], Entry::Separator));
        let Entry::Item(no_action) = &nested.entries()[3] else {
            panic!("inert item")
        };
        assert!(!no_action.is_enabled());
        assert!(no_action.action().is_none());
        for (entry, command) in bar.menus()[1].entries().iter().zip([
            Some(edit::Command::Cut),
            Some(edit::Command::Copy),
            Some(edit::Command::Paste),
            None,
            Some(edit::Command::SelectAll),
        ]) {
            match (entry, command) {
                (Entry::Item(item), Some(command)) => assert!(
                    matches!(item.action(), Some(Action::Edit(actual)) if *actual == command)
                ),
                (Entry::Separator, None) => {}
                _ => panic!("standard edit menu structure"),
            }
        }
    }

    #[test]
    fn absent_messages_disable_items_without_reviving_explicitly_disabled_ones() {
        let absent = Item::new("Absent").on_press_maybe(None::<u8>);
        assert!(!absent.is_enabled());
        let present = absent.on_press_maybe(Some(7));
        assert!(present.is_enabled());
        let disabled = present.enabled(false).on_press(8).map(u16::from);
        assert!(!disabled.is_enabled());
    }
}
