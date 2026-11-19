use std::fmt;

use crate::menu::map::Map;
use crate::menu::{Action, Output};

use super::entry::Entry;
use super::prefix::Prefix;

/// The owned actions associated with a menu or bar description.
///
/// Obtain this together with its description using [`Menu::into_parts`](crate::menu::Menu::into_parts)
/// or [`Bar::into_parts`](crate::menu::Bar::into_parts). A runtime must retain the
/// originating dispatch while any native callback can use its entry paths.
pub struct Dispatch<Message> {
    prefix: Option<Prefix<Message>>,
    entries: Vec<Entry<Message>>,
}

impl<Message> fmt::Debug for Dispatch<Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dispatch")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

impl<Message> Default for Dispatch<Message> {
    fn default() -> Self {
        Self {
            prefix: None,
            entries: Vec::new(),
        }
    }
}

impl<Message> Dispatch<Message> {
    /// Activates the item selected by its zero-based entry path.
    ///
    /// A bar path begins with the top-level menu index. A menu path begins with
    /// its first entry index. Each subsequent index selects an entry in a submenu.
    /// Separators, missing actions, and invalid paths return `None`.
    ///
    /// This invokes the action directly. The runtime must first check the
    /// description's enabled state and its window/focus/lifetime policy.
    pub fn activate(&self, path: &[usize]) -> Option<Output<Message>> {
        let (&index, path) = path.split_first()?;
        self.activate_at(index, path)
    }

    pub(in crate::menu) fn activate_at(
        &self,
        mut index: usize,
        path: &[usize],
    ) -> Option<Output<Message>> {
        if let Some(prefix) = &self.prefix {
            if index < prefix.len {
                return (prefix.activate)(index, path);
            }
            index -= prefix.len;
        }
        match self.entries.get(index)? {
            Entry::Item(Some(action)) if path.is_empty() => Some(match action {
                Action::Message(produce) => Output::Message(produce()),
                Action::Edit(command) => Output::Edit(*command),
            }),
            Entry::Menu(menu) => menu.activate(path),
            Entry::Item(_) | Entry::Separator => None,
        }
    }

    pub(in crate::menu) fn len(&self) -> usize {
        self.prefix.as_ref().map_or(0, |prefix| prefix.len) + self.entries.len()
    }

    pub(in crate::menu) fn push(&mut self, entry: Entry<Message>) {
        self.entries.push(entry);
    }

    pub(in crate::menu) fn map<Other>(
        self,
        mapper: impl Fn(Message) -> Other + 'static,
    ) -> Dispatch<Other>
    where
        Message: 'static,
        Other: 'static,
    {
        let len = self.len();
        let mapping = Map::new(self, mapper);
        Dispatch {
            prefix: Some(Prefix {
                len,
                activate: Box::new(move |index, path| mapping.activate(index, path)),
            }),
            entries: Vec::new(),
        }
    }
}
