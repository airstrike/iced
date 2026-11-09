//! Native application menus.
//!
//! Build a [`Bar`] from menus and return it from your program's menu function.
//! Menu actions are evaluated only when the user activates an enabled item.
//! Native menus are currently supported on macOS; requesting a nonempty bar on
//! another platform is not yet implemented. An empty bar preserves the platform's
//! default application menu.

mod bar;
mod construction;
mod entry;
mod item;
pub mod shortcut;

pub use bar::Bar;
pub use construction::{bar, edit, item, separator, submenu};
pub use entry::Entry;
pub use item::{Action, Item};
pub use shortcut::Shortcut;

use std::rc::Rc;

/// A named menu containing items, separators, and nested menus.
#[derive(Debug)]
pub struct Menu<Message> {
    title: String,
    entries: Vec<Entry<Message>>,
    enabled: bool,
}

impl<Message> Clone for Menu<Message> {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            entries: self.entries.clone(),
            enabled: self.enabled,
        }
    }
}

impl<Message> Menu<Message> {
    /// Creates an empty, enabled menu with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entries: Vec::new(),
            enabled: true,
        }
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

    fn map_with<Other>(self, f: Rc<dyn Fn(Message) -> Other>) -> Menu<Other>
    where
        Message: 'static,
        Other: 'static,
    {
        Menu {
            title: self.title,
            enabled: self.enabled,
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
