use super::{Dispatch, Entry, Role, description};

/// A named menu containing items, separators, and nested menus.
#[derive(Debug)]
pub struct Menu<Message> {
    description: description::Menu,
    dispatch: Dispatch<Message>,
}

impl<Message> Menu<Message> {
    /// Creates an empty, enabled menu with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            description: description::Menu {
                title: title.into(),
                entries: Vec::new(),
                enabled: true,
                role: None,
            },
            dispatch: Dispatch::default(),
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
        for entry in entries {
            self = self.push(entry);
        }
        self
    }

    /// Appends an entry to the menu.
    pub fn push(mut self, entry: impl Into<Entry<Message>>) -> Self {
        let (description, dispatch) = entry.into().into_parts();
        self.description.entries.push(description);
        self.dispatch.push(dispatch);
        self
    }

    /// Sets whether this menu and its descendants can be activated.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.description.enabled = enabled;
        self
    }

    /// Assigns a native role to this menu, independently of its title.
    ///
    /// The first top-level menu with each role is registered with the platform.
    /// Roles on nested menus and later declarations of the same role are ignored.
    /// Native menus currently support macOS only.
    pub fn role(mut self, role: Role) -> Self {
        self.description.role = Some(role);
        self
    }

    /// Returns the native role assigned to this menu, if any.
    pub fn native_role(&self) -> Option<Role> {
        self.description.native_role()
    }

    /// Returns the menu title.
    pub fn title(&self) -> &str {
        self.description.title()
    }

    /// Returns the message-independent entries in display order.
    ///
    /// For runtime lowering, use [`Self::into_parts`] instead of consuming entries
    /// individually: a mapped menu owns one mapper shared by its action dispatch.
    pub fn entries(&self) -> &[description::Entry] {
        self.description.entries()
    }

    /// Returns whether this menu permits activation of its descendants.
    pub fn is_enabled(&self) -> bool {
        self.description.is_enabled()
    }

    /// Separates presentation from its originating action dispatch for a runtime.
    pub fn into_parts(self) -> (description::Menu, Dispatch<Message>) {
        (self.description, self.dispatch)
    }

    /// Transforms emitted messages lazily, preserving semantic edit actions.
    ///
    /// One adapter owns the mapper around the original menu. Mapping does not
    /// visit item factories, and the mapper does not need to implement `Clone`.
    pub fn map<Other>(self, mapper: impl Fn(Message) -> Other + 'static) -> Menu<Other>
    where
        Message: 'static,
        Other: 'static,
    {
        Menu {
            description: self.description,
            dispatch: self.dispatch.map(mapper),
        }
    }
}
