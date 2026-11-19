use crate::menu::{Icon, Shortcut};

use super::Action;

/// The presentation of a menu item, without its message factory.
#[derive(Debug, Clone)]
pub struct Item {
    pub(crate) title: String,
    pub(crate) enabled: bool,
    pub(crate) checked: bool,
    pub(crate) shortcut: Option<Shortcut>,
    pub(crate) icon: Option<Icon>,
    pub(crate) action: Option<Action>,
}

impl Item {
    /// Returns the item title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns whether the item has an action and is individually enabled.
    ///
    /// Disabled ancestor menus also prevent native activation.
    pub fn is_enabled(&self) -> bool {
        self.enabled && self.action.is_some()
    }

    /// Returns whether the item displays a check mark.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Returns the image displayed beside this item, if any.
    pub fn image(&self) -> Option<&Icon> {
        self.icon.as_ref()
    }

    /// Returns the validated shortcut, if any.
    pub fn shortcut_key(&self) -> Option<&Shortcut> {
        self.shortcut.as_ref()
    }

    /// Returns the action kind without producing its message.
    pub fn action(&self) -> Option<Action> {
        self.action
    }
}
