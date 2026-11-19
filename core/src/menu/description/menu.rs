use crate::menu::Role;

use super::Entry;

/// The presentation of a named native menu, without its message factories.
#[derive(Debug, Clone)]
pub struct Menu {
    pub(crate) title: String,
    pub(crate) entries: Vec<Entry>,
    pub(crate) enabled: bool,
    pub(crate) role: Option<Role>,
}

impl Menu {
    /// Returns the native role assigned to this menu, if any.
    pub fn native_role(&self) -> Option<Role> {
        self.role
    }

    /// Returns the menu title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns entries in display order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Returns whether this menu permits activation of its descendants.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}
