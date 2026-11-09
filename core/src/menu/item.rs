use super::Action;
use crate::edit;
use crate::keyboard::Modifiers;
use crate::menu::shortcut::{self, Shortcut};

/// A native menu item with an optional action and shortcut.
#[derive(Debug)]
pub struct Item<Message> {
    title: String,
    action: Option<Action<Message>>,
    enabled: bool,
    checked: bool,
    shortcut: Option<Shortcut>,
}

impl<Message> Item<Message> {
    /// Creates an item without an action. It is disabled until an action is set.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            action: None,
            enabled: true,
            checked: false,
            shortcut: None,
        }
    }

    /// Creates a semantic editing item with its conventional title and shortcut.
    pub fn edit(command: edit::Command) -> Self {
        let (title, key) = match command {
            edit::Command::Cut => ("Cut", 'x'),
            edit::Command::Copy => ("Copy", 'c'),
            edit::Command::Paste => ("Paste", 'v'),
            edit::Command::SelectAll => ("Select All", 'a'),
        };

        Self {
            action: Some(Action::Edit(command)),
            ..Self::new(title)
        }
        .shortcut(Modifiers::COMMAND, key)
    }

    /// Emits a clone of the given message when this item is activated.
    pub fn on_press(self, message: Message) -> Self
    where
        Message: Clone + 'static,
    {
        self.on_press_with(move || message.clone())
    }

    /// Sets the message emitted on activation, or disables an item with no message.
    pub fn on_press_maybe(mut self, message: Option<Message>) -> Self
    where
        Message: Clone + 'static,
    {
        self.action = message.map(|message| Action::Message(Box::new(move || message.clone())));
        self
    }

    /// Produces a fresh message when activated, without requiring messages to clone.
    ///
    /// The factory is not called while constructing, mapping, or displaying menus.
    pub fn on_press_with(mut self, f: impl Fn() -> Message + 'static) -> Self {
        self.action = Some(Action::Message(Box::new(f)));
        self
    }

    /// Sets whether this item can be activated. An action must also be present.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the check mark. Activating an item does not toggle it automatically.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Sets this item's shortcut.
    ///
    /// # Panics
    /// Panics if the key or modifiers are rejected by [`Shortcut::new`]. Use
    /// [`Self::try_shortcut`] to handle invalid shortcuts without panicking.
    pub fn shortcut(self, modifiers: Modifiers, key: char) -> Self {
        self.try_shortcut(modifiers, key)
            .expect("native menu shortcut must be valid")
    }

    /// Sets this item's shortcut after validating the key and modifiers.
    pub fn try_shortcut(
        mut self,
        modifiers: Modifiers,
        key: char,
    ) -> Result<Self, shortcut::Error> {
        self.shortcut = Some(Shortcut::new(modifiers, key)?);
        Ok(self)
    }

    /// Returns the item title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns whether this item has an action and is individually enabled.
    ///
    /// Disabled ancestor menus also prevent activation.
    pub fn is_enabled(&self) -> bool {
        self.enabled && self.action.is_some()
    }

    /// Returns whether this item displays a check mark.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Returns the validated shortcut, if any.
    pub fn shortcut_key(&self) -> Option<Shortcut> {
        self.shortcut
    }

    /// Returns the activation action, if any.
    pub fn action(&self) -> Option<&Action<Message>> {
        self.action.as_ref()
    }

    /// Consumes the item and transfers its action to a runtime.
    pub fn into_action(self) -> Option<Action<Message>> {
        self.action
    }

    /// Transforms emitted messages lazily, preserving semantic edit actions.
    pub fn map<Other>(self, f: impl Fn(Message) -> Other + 'static) -> Item<Other>
    where
        Message: 'static,
        Other: 'static,
    {
        Item {
            title: self.title,
            action: self.action.map(|action| match action {
                Action::Message(produce) => Action::Message(Box::new(move || f(produce()))),
                Action::Edit(command) => Action::Edit(command),
            }),
            enabled: self.enabled,
            checked: self.checked,
            shortcut: self.shortcut,
        }
    }
}
