//! Validated keyboard shortcuts for native menus.
use crate::keyboard::key::Named;
use crate::keyboard::{Key, Modifiers};

/// A validated logical key and its modifiers for a native menu item.
///
/// Character keys require Control, Alt, or Logo. Supported named keys, including
/// function keys, can be used without modifiers. ASCII uppercase characters are
/// normalized to lowercase with [`Modifiers::SHIFT`]; other Unicode characters
/// are preserved as written.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    modifiers: Modifiers,
    key: Key,
}

/// A rejected native menu shortcut.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The key cannot be represented by a native menu shortcut.
    #[error("menu shortcut key {key:?} is not supported")]
    UnsupportedKey {
        /// The rejected key.
        key: Key,
    },
    /// The modifiers contain unsupported bits.
    #[error("menu shortcut modifiers contain unknown bits: {bits:#x}")]
    UnknownModifiers {
        /// The unsupported modifier bits.
        bits: u32,
    },
    /// A character shortcut has no Control, Alt, or Logo modifier.
    #[error("menu shortcut {key:?} requires Control, Alt, or Logo; received {modifiers:?}")]
    MissingCommandModifier {
        /// The rejected key.
        key: Key,
        /// The rejected modifiers.
        modifiers: Modifiers,
    },
}

impl Shortcut {
    /// Creates a validated native menu shortcut.
    ///
    /// Accepts a character, string, [`Key`], or [`Named`] key. Character keys must
    /// contain one Unicode scalar and require Control, Alt, or Logo. Supported
    /// named keys are Enter, Tab, Escape, Space, navigation/editing keys, and
    /// F1–F35. Space follows the character-key modifier rule.
    ///
    /// # Errors
    /// Returns [`Error::UnsupportedKey`] for unidentified keys, unsupported named
    /// keys, empty/multiple characters, control characters, or characters in
    /// AppKit's reserved function-key range. Returns [`Error::UnknownModifiers`]
    /// for unsupported modifier bits and [`Error::MissingCommandModifier`] for
    /// character shortcuts without a command modifier.
    pub fn new(mut modifiers: Modifiers, key: impl Into<Key>) -> Result<Self, Error> {
        let mut key = key.into();
        let unknown = modifiers.bits() & !Modifiers::all().bits();
        if unknown != 0 {
            return Err(Error::UnknownModifiers { bits: unknown });
        }

        if key == Key::Named(Named::Space) {
            key = ' '.into();
        }

        match &mut key {
            Key::Character(text) => {
                let mut characters = text.chars();
                let Some(character) = characters.next() else {
                    return Err(Error::UnsupportedKey { key });
                };
                if characters.next().is_some()
                    || character.is_control()
                    || ('\u{f700}'..='\u{f8ff}').contains(&character)
                {
                    return Err(Error::UnsupportedKey { key });
                }
                if !modifiers.intersects(Modifiers::CTRL | Modifiers::ALT | Modifiers::LOGO) {
                    return Err(Error::MissingCommandModifier { key, modifiers });
                }
                if character.is_ascii_uppercase() {
                    modifiers |= Modifiers::SHIFT;
                    *text = character.to_ascii_lowercase().to_string().into();
                }
            }
            Key::Named(
                Named::Enter
                | Named::Tab
                | Named::Escape
                | Named::ArrowUp
                | Named::ArrowDown
                | Named::ArrowLeft
                | Named::ArrowRight
                | Named::Home
                | Named::End
                | Named::PageUp
                | Named::PageDown
                | Named::Backspace
                | Named::Delete
                | Named::Insert
                | Named::Clear
                | Named::Undo
                | Named::Redo
                | Named::Find
                | Named::Help
                | Named::Select
                | Named::ContextMenu
                | Named::Pause
                | Named::PrintScreen
                | Named::ScrollLock
                | Named::F1
                | Named::F2
                | Named::F3
                | Named::F4
                | Named::F5
                | Named::F6
                | Named::F7
                | Named::F8
                | Named::F9
                | Named::F10
                | Named::F11
                | Named::F12
                | Named::F13
                | Named::F14
                | Named::F15
                | Named::F16
                | Named::F17
                | Named::F18
                | Named::F19
                | Named::F20
                | Named::F21
                | Named::F22
                | Named::F23
                | Named::F24
                | Named::F25
                | Named::F26
                | Named::F27
                | Named::F28
                | Named::F29
                | Named::F30
                | Named::F31
                | Named::F32
                | Named::F33
                | Named::F34
                | Named::F35,
            ) => {}
            Key::Named(_) | Key::Unidentified => return Err(Error::UnsupportedKey { key }),
        }

        Ok(Self { modifiers, key })
    }

    /// Returns the normalized shortcut modifiers.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Returns the normalized logical key.
    pub fn key(&self) -> &Key {
        &self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uppercase_ascii_and_space_aliases_are_normalized() {
        assert_eq!(
            Shortcut::new(Modifiers::LOGO, 'S'),
            Shortcut::new(Modifiers::LOGO | Modifiers::SHIFT, 's'),
        );
        assert_eq!(
            Shortcut::new(Modifiers::LOGO, Named::Space),
            Shortcut::new(Modifiers::LOGO, ' '),
        );
    }

    #[test]
    fn unicode_characters_are_preserved_without_case_folding() {
        for character in ['é', 'É', 'ß', 'İ', '中', '🦀'] {
            let shortcut = Shortcut::new(Modifiers::LOGO, character).unwrap();
            assert_eq!(shortcut.key(), &Key::from(character));
            assert_eq!(shortcut.modifiers(), Modifiers::LOGO);
            assert_eq!(
                Shortcut::new(Modifiers::LOGO, character.to_string()),
                Ok(shortcut)
            );
        }
    }

    #[test]
    fn named_keys_can_be_used_without_command_modifiers() {
        for key in [
            Named::F1,
            Named::F35,
            Named::ArrowUp,
            Named::Backspace,
            Named::Escape,
        ] {
            assert!(Shortcut::new(Modifiers::NONE, key).is_ok());
            assert!(Shortcut::new(Modifiers::SHIFT, key).is_ok());
        }
        for key in [Key::from('a'), Key::from('é'), Key::from(Named::Space)] {
            assert!(matches!(
                Shortcut::new(Modifiers::NONE, key),
                Err(Error::MissingCommandModifier { .. }),
            ));
        }
    }

    #[test]
    fn invalid_characters_keys_and_modifiers_are_rejected() {
        for key in ["", "ab", "e\u{301}", "\n", "\u{7f}", "\u{f704}"] {
            assert_eq!(
                Shortcut::new(Modifiers::CTRL, key),
                Err(Error::UnsupportedKey { key: key.into() })
            );
        }
        for key in [
            Key::Unidentified,
            Key::from(Named::Shift),
            Key::from(Named::AudioVolumeUp),
        ] {
            assert_eq!(
                Shortcut::new(Modifiers::CTRL, key.clone()),
                Err(Error::UnsupportedKey { key })
            );
        }
        assert_eq!(
            Shortcut::new(Modifiers::from_bits_retain(1) | Modifiers::CTRL, 'a'),
            Err(Error::UnknownModifiers { bits: 1 }),
        );
    }
}
