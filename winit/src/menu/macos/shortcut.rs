use std::collections::HashSet;

use objc2_app_kit::{NSEventModifierFlags, NSMenuItem};

use crate::core::keyboard::key::Named;
use crate::core::{keyboard, menu};

pub(super) fn modifiers(modifiers: keyboard::Modifiers) -> NSEventModifierFlags {
    let mut flags = NSEventModifierFlags::empty();
    if modifiers.control() {
        flags |= NSEventModifierFlags::Control;
    }
    if modifiers.alt() {
        flags |= NSEventModifierFlags::Option;
    }
    if modifiers.logo() {
        flags |= NSEventModifierFlags::Command;
    }
    if modifiers.shift() {
        flags |= NSEventModifierFlags::Shift;
    }
    flags
}

// AppKit key equivalents use Unicode control characters and the function-key
// constants defined by NSEvent. Keep encoding and inherited-menu decoding paired.
const NAMED: &[(Named, &str)] = &[
    (Named::Enter, "\u{d}"),
    (Named::Tab, "\u{9}"),
    (Named::Escape, "\u{1b}"),
    (Named::ArrowUp, "\u{f700}"),
    (Named::ArrowDown, "\u{f701}"),
    (Named::ArrowLeft, "\u{f702}"),
    (Named::ArrowRight, "\u{f703}"),
    (Named::Home, "\u{f729}"),
    (Named::End, "\u{f72b}"),
    (Named::PageUp, "\u{f72c}"),
    (Named::PageDown, "\u{f72d}"),
    (Named::Backspace, "\u{8}"),
    (Named::Delete, "\u{7f}"),
    (Named::Insert, "\u{f727}"),
    (Named::Clear, "\u{c}"),
    (Named::Undo, "\u{f743}"),
    (Named::Redo, "\u{f744}"),
    (Named::Find, "\u{f745}"),
    (Named::Help, "\u{f746}"),
    (Named::Select, "\u{f741}"),
    (Named::ContextMenu, "\u{f735}"),
    (Named::Pause, "\u{f730}"),
    (Named::PrintScreen, "\u{f72e}"),
    (Named::ScrollLock, "\u{f72f}"),
    (Named::F1, "\u{f704}"),
    (Named::F2, "\u{f705}"),
    (Named::F3, "\u{f706}"),
    (Named::F4, "\u{f707}"),
    (Named::F5, "\u{f708}"),
    (Named::F6, "\u{f709}"),
    (Named::F7, "\u{f70a}"),
    (Named::F8, "\u{f70b}"),
    (Named::F9, "\u{f70c}"),
    (Named::F10, "\u{f70d}"),
    (Named::F11, "\u{f70e}"),
    (Named::F12, "\u{f70f}"),
    (Named::F13, "\u{f710}"),
    (Named::F14, "\u{f711}"),
    (Named::F15, "\u{f712}"),
    (Named::F16, "\u{f713}"),
    (Named::F17, "\u{f714}"),
    (Named::F18, "\u{f715}"),
    (Named::F19, "\u{f716}"),
    (Named::F20, "\u{f717}"),
    (Named::F21, "\u{f718}"),
    (Named::F22, "\u{f719}"),
    (Named::F23, "\u{f71a}"),
    (Named::F24, "\u{f71b}"),
    (Named::F25, "\u{f71c}"),
    (Named::F26, "\u{f71d}"),
    (Named::F27, "\u{f71e}"),
    (Named::F28, "\u{f71f}"),
    (Named::F29, "\u{f720}"),
    (Named::F30, "\u{f721}"),
    (Named::F31, "\u{f722}"),
    (Named::F32, "\u{f723}"),
    (Named::F33, "\u{f724}"),
    (Named::F34, "\u{f725}"),
    (Named::F35, "\u{f726}"),
];

pub(super) fn key(shortcut: &menu::Shortcut) -> String {
    match shortcut.key() {
        keyboard::Key::Character(character) => character.to_string(),
        keyboard::Key::Named(named) => NAMED
            .iter()
            .find_map(|(key, equivalent)| (key == named).then_some(*equivalent))
            .expect("Shortcut validates the named keys supported by native menus")
            .to_owned(),
        keyboard::Key::Unidentified => unreachable!("Shortcut rejects unidentified keys"),
    }
}

fn decode(key: &str, modifiers: keyboard::Modifiers) -> Option<menu::Shortcut> {
    let named = NAMED
        .iter()
        .find_map(|(named, equivalent)| (*equivalent == key).then_some(*named));
    let key = named.map_or_else(|| keyboard::Key::from(key), keyboard::Key::from);
    menu::Shortcut::new(modifiers, key).ok()
}

pub(super) fn collect(item: &NSMenuItem, shortcuts: &mut HashSet<menu::Shortcut>) {
    let flags = item.keyEquivalentModifierMask();
    let mut modifiers = keyboard::Modifiers::empty();
    if flags.contains(NSEventModifierFlags::Control) {
        modifiers |= keyboard::Modifiers::CTRL;
    }
    if flags.contains(NSEventModifierFlags::Option) {
        modifiers |= keyboard::Modifiers::ALT;
    }
    if flags.contains(NSEventModifierFlags::Command) {
        modifiers |= keyboard::Modifiers::LOGO;
    }
    if flags.contains(NSEventModifierFlags::Shift) {
        modifiers |= keyboard::Modifiers::SHIFT;
    }
    if let Some(shortcut) = decode(&item.keyEquivalent().to_string(), modifiers) {
        let _ = shortcuts.insert(shortcut);
    }
    if let Some(menu) = item.submenu() {
        for item in menu.itemArray() {
            collect(&item, shortcuts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyboard::Modifiers;

    #[test]
    fn all_supported_native_named_keys_round_trip() {
        for &(named, equivalent) in NAMED {
            let shortcut = menu::Shortcut::new(Modifiers::NONE, named).unwrap();
            assert_eq!(key(&shortcut), equivalent);
            assert_eq!(decode(equivalent, Modifiers::NONE), Some(shortcut));
        }
    }

    #[test]
    fn native_unicode_and_function_keys_keep_their_identity() {
        let unicode = menu::Shortcut::new(Modifiers::COMMAND, 'é').unwrap();
        assert_eq!(key(&unicode), "é");
        assert_eq!(decode("é", Modifiers::COMMAND), Some(unicode));
        let function = menu::Shortcut::new(Modifiers::SHIFT, Named::F5).unwrap();
        assert_eq!(key(&function), "\u{f708}");
        assert_eq!(decode("\u{f708}", Modifiers::SHIFT), Some(function));
        assert_eq!(decode("", Modifiers::COMMAND), None);
        assert_eq!(decode("ab", Modifiers::COMMAND), None);
    }

    #[test]
    fn named_equivalents_match_appkit_constants() {
        let cases = [
            (Named::F1, objc2_app_kit::NSF1FunctionKey),
            (Named::F35, objc2_app_kit::NSF35FunctionKey),
            (Named::ArrowLeft, objc2_app_kit::NSLeftArrowFunctionKey),
            (Named::PageDown, objc2_app_kit::NSPageDownFunctionKey),
        ];
        for (named, scalar) in cases {
            let shortcut = menu::Shortcut::new(Modifiers::NONE, named).unwrap();
            assert_eq!(key(&shortcut), char::from_u32(scalar).unwrap().to_string());
        }
        // NSMenuItem.keyEquivalent documents these independently of NSEvent.
        assert_eq!(
            key(&menu::Shortcut::new(Modifiers::NONE, Named::Backspace).unwrap()),
            "\u{8}"
        );
        assert_eq!(
            key(&menu::Shortcut::new(Modifiers::NONE, Named::Delete).unwrap()),
            "\u{7f}"
        );
    }

    #[test]
    fn inherited_key_equivalents_conflict_with_declarations() {
        let mut claimed = HashSet::new();
        let _ = claimed.insert(decode("\u{f708}", Modifiers::NONE).unwrap());
        let _ = claimed.insert(decode("é", Modifiers::COMMAND).unwrap());
        let _ = claimed.insert(decode("S", Modifiers::COMMAND).unwrap());
        assert!(!claimed.insert(menu::Shortcut::new(Modifiers::NONE, Named::F5).unwrap()));
        assert!(!claimed.insert(menu::Shortcut::new(Modifiers::COMMAND, 'é').unwrap()));
        assert!(
            !claimed
                .insert(menu::Shortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, 's').unwrap())
        );
    }
}
