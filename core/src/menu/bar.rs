use super::{Dispatch, Menu, description, dispatch};

/// The application menus contributed by a program.
#[derive(Debug)]
pub struct Bar<Message> {
    menus: Vec<description::Menu>,
    dispatch: Dispatch<Message>,
}

impl<Message> Default for Bar<Message> {
    fn default() -> Self {
        Self {
            menus: Vec::new(),
            dispatch: Dispatch::default(),
        }
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

impl<Message> Bar<Message> {
    /// Creates a menu bar from the given menus, in display order.
    pub fn new(menus: impl IntoIterator<Item = Menu<Message>>) -> Self {
        let mut bar = Self::default();
        for menu in menus {
            bar = bar.push(menu);
        }
        bar
    }

    /// Creates a menu bar from an owned vector of menus.
    pub fn from_vec(menus: Vec<Menu<Message>>) -> Self {
        Self::new(menus)
    }

    /// Appends a menu to the bar.
    pub fn push(mut self, menu: Menu<Message>) -> Self {
        let (description, actions) = menu.into_parts();
        self.menus.push(description);
        self.dispatch.push(dispatch::Entry::Menu(actions));
        self
    }

    /// Sets whether every top-level menu and its descendants can be activated.
    pub fn enabled(mut self, enabled: bool) -> Self {
        for menu in &mut self.menus {
            menu.enabled = enabled;
        }
        self
    }

    /// Returns the message-independent menu descriptions in display order.
    ///
    /// For runtime lowering, use [`Self::into_parts`] instead of consuming menus
    /// individually: a mapped bar owns one mapper around its action dispatch.
    pub fn menus(&self) -> &[description::Menu] {
        &self.menus
    }

    /// Returns whether the bar contributes no menus.
    pub fn is_empty(&self) -> bool {
        self.menus.is_empty()
    }

    /// Separates presentation from its originating action dispatch for a runtime.
    pub fn into_parts(self) -> (Vec<description::Menu>, Dispatch<Message>) {
        (self.menus, self.dispatch)
    }

    /// Transforms emitted messages lazily, preserving semantic edit actions.
    ///
    /// One adapter owns the mapper around all menus in this bar.
    pub fn map<Other>(self, mapper: impl Fn(Message) -> Other + 'static) -> Bar<Other>
    where
        Message: 'static,
        Other: 'static,
    {
        Bar {
            menus: self.menus,
            dispatch: self.dispatch.map(mapper),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit;
    use crate::keyboard::Modifiers;
    use crate::menu::description::{Action, Entry};
    use crate::menu::{self, Item, Output};

    use std::cell::Cell;
    use std::rc::Rc;

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
        let (menus, dispatch) = bar.into_parts();
        let Entry::Submenu(nested) = &menus[0].entries()[0] else {
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
        assert_eq!(item.action(), Some(Action::Message));
        for expected in 1..=2 {
            let Some(Output::Message(Other(actual))) = dispatch.activate(&[0, 0, 0]) else {
                panic!("message factory")
            };
            assert_eq!(actual, expected);
        }
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
        for (entry, command) in menus[1].entries().iter().zip([
            Some(edit::Command::Cut),
            Some(edit::Command::Copy),
            Some(edit::Command::Paste),
            None,
            Some(edit::Command::SelectAll),
        ]) {
            match (entry, command) {
                (Entry::Item(item), Some(command)) => assert!(
                    matches!(item.action(), Some(Action::Edit(actual)) if actual == command)
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
