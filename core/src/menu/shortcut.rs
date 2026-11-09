//! Validated keyboard shortcuts for native menus.
use crate::keyboard::Modifiers;

/// A printable ASCII key combined with a command modifier.
///
/// Construction validates the key and modifiers. Uppercase letters are
/// normalized to lowercase with [`Modifiers::SHIFT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    modifiers: Modifiers,
    key: char,
}

/// A rejected native menu shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The key is not printable ASCII.
    #[error("menu shortcut key {key:?} is not printable ASCII")]
    InvalidKey {
        /// The rejected key.
        key: char,
    },
    /// The modifiers contain unsupported bits.
    #[error("menu shortcut modifiers contain unknown bits: {bits:#x}")]
    UnknownModifiers {
        /// The unsupported modifier bits.
        bits: u32,
    },
    /// The shortcut has no Control, Alt, or Logo modifier.
    #[error("menu shortcut {key:?} requires Control, Alt, or Logo; received {modifiers:?}")]
    MissingCommandModifier {
        /// The rejected key.
        key: char,
        /// The rejected modifiers.
        modifiers: Modifiers,
    },
}

impl Shortcut {
    /// Creates a validated native menu shortcut.
    ///
    /// Keys must be printable ASCII, and modifiers must include at least one of
    /// Control, Alt, or Logo. Unknown modifier bits are rejected.
    pub fn new(mut modifiers: Modifiers, key: char) -> Result<Self, Error> {
        if !(' '..='~').contains(&key) {
            return Err(Error::InvalidKey { key });
        }

        let unknown = modifiers.bits() & !Modifiers::all().bits();
        if unknown != 0 {
            return Err(Error::UnknownModifiers { bits: unknown });
        }

        if !modifiers.intersects(Modifiers::CTRL | Modifiers::ALT | Modifiers::LOGO) {
            return Err(Error::MissingCommandModifier { key, modifiers });
        }

        if key.is_ascii_uppercase() {
            modifiers |= Modifiers::SHIFT;
        }

        Ok(Self {
            modifiers,
            key: key.to_ascii_lowercase(),
        })
    }

    /// Returns the normalized shortcut modifiers.
    pub fn modifiers(self) -> Modifiers {
        self.modifiers
    }

    /// Returns the normalized printable ASCII key.
    pub fn key(self) -> char {
        self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uppercase_keys_are_normalized_with_shift() {
        assert_eq!(
            Shortcut::new(Modifiers::LOGO, 'S'),
            Shortcut::new(Modifiers::LOGO | Modifiers::SHIFT, 's'),
        );
    }

    #[test]
    fn unmodified_and_shift_only_shortcuts_are_rejected() {
        for modifiers in [Modifiers::NONE, Modifiers::SHIFT] {
            assert!(matches!(
                Shortcut::new(modifiers, 'a'),
                Err(Error::MissingCommandModifier { .. })
            ));
        }
    }

    #[test]
    fn nonprintable_nonascii_and_unknown_modifier_bits_are_rejected() {
        for key in ['\n', '\u{7f}', 'é', '🦀'] {
            assert_eq!(
                Shortcut::new(Modifiers::CTRL, key),
                Err(Error::InvalidKey { key })
            );
        }
        assert_eq!(
            Shortcut::new(Modifiers::from_bits_retain(1) | Modifiers::CTRL, 'a'),
            Err(Error::UnknownModifiers { bits: 1 })
        );
        assert!(Shortcut::new(Modifiers::ALT, ' ').is_ok());
        assert!(Shortcut::new(Modifiers::LOGO, '~').is_ok());
    }
}
