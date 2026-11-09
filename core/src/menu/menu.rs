use std::rc::Rc;

use super::{Entry, Role};

/// A named menu containing items, separators, and nested menus.
#[derive(Debug)]
pub struct Menu<Message> {
    title: String,
    entries: Vec<Entry<Message>>,
    enabled: bool,
    role: Option<Role>,
}

impl<Message> IntoIterator for Menu<Message> {
    type Item = Entry<Message>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<Message> Menu<Message> {
    /// Creates an empty, enabled menu with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entries: Vec::new(),
            enabled: true,
            role: None,
        }
    }

    /// Creates an enabled menu from items, submenus, or entries.
    pub fn with_entries(
        title: impl Into<String>,
        entries: impl IntoIterator<Item = impl Into<Entry<Message>>>,
    ) -> Self {
        Self::new(title).extend(entries)
    }

    /// Appends entries from an iterator, preserving their order.
    pub fn extend(mut self, entries: impl IntoIterator<Item = impl Into<Entry<Message>>>) -> Self {
        self.entries.extend(entries.into_iter().map(Into::into));
        self
    }

    /// Appends an entry to the menu.
    pub fn push(mut self, entry: impl Into<Entry<Message>>) -> Self {
        self.entries.push(entry.into());
        self
    }

    /// Sets whether this menu and its descendants can be activated.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Assigns a native role to this menu, independently of its title.
    ///
    /// The first top-level menu with each role is registered with the platform.
    /// Roles on nested menus and later declarations of the same role are ignored.
    /// Native menus currently support macOS only.
    pub fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    /// Returns the native role assigned to this menu, if any.
    pub fn native_role(&self) -> Option<Role> {
        self.role
    }

    /// Returns the menu title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the entries in display order.
    pub fn entries(&self) -> &[Entry<Message>] {
        &self.entries
    }

    /// Returns whether this menu permits activation of its descendants.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Transforms emitted messages lazily, preserving semantic edit actions.
    pub fn map<Other>(self, f: impl Fn(Message) -> Other + 'static) -> Menu<Other>
    where
        Message: 'static,
        Other: 'static,
    {
        self.map_with(Rc::new(f))
    }

    pub(super) fn map_with<Other>(self, f: Rc<dyn Fn(Message) -> Other>) -> Menu<Other>
    where
        Message: 'static,
        Other: 'static,
    {
        Menu {
            title: self.title,
            enabled: self.enabled,
            role: self.role,
            entries: self
                .entries
                .into_iter()
                .map(|entry| {
                    let f = Rc::clone(&f);
                    entry.map_with(f)
                })
                .collect(),
        }
    }
}
