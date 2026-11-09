use std::collections::HashSet;

use objc2_app_kit::{NSEventModifierFlags, NSMenuItem};

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

pub(super) fn collect(item: &NSMenuItem, shortcuts: &mut HashSet<menu::Shortcut>) {
    let key = item.keyEquivalent().to_string();
    let mut chars = key.chars();
    if let Some(key) = chars.next()
        && chars.next().is_none()
    {
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
        if let Ok(shortcut) = menu::Shortcut::new(modifiers, key) {
            let _ = shortcuts.insert(shortcut);
        }
    }
    if let Some(menu) = item.submenu() {
        for item in menu.itemArray() {
            collect(&item, shortcuts);
        }
    }
}
