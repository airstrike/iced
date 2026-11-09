use objc2::DefinedClass;
use objc2::rc::Retained;
use objc2_app_kit::NSMenuItem;
use objc2_foundation::{MainThreadMarker, NSString, ns_string};

use crate::menu::branch;

use super::delegate::Delegate;
use super::shortcut;
use super::target::Target;

pub(super) struct Item {
    pub(super) item: Retained<NSMenuItem>,
    pub(super) target: Option<Retained<Target>>,
}

impl Item {
    pub(super) fn update(&mut self, snapshot: branch::Item, delegate: &Delegate) {
        self.item.setTitle(&NSString::from_str(&snapshot.title));
        self.item.setEnabled(snapshot.enabled);
        self.item.setState(if snapshot.checked { 1 } else { 0 });

        if let Some(shortcut) = snapshot.shortcut {
            self.item
                .setKeyEquivalent(&NSString::from_str(&shortcut::key(&shortcut)));
            self.item
                .setKeyEquivalentModifierMask(shortcut::modifiers(shortcut.modifiers()));
        } else {
            self.item.setKeyEquivalent(ns_string!(""));
        }

        self.clear_target();
        self.target = snapshot.callback.map(|callback| {
            let target = Target::new(MainThreadMarker::from(&*self.item), callback, delegate);
            // SAFETY: The selector is Target::activate:, and self.target retains
            // this object until clear_target disconnects AppKit's weak pointer.
            unsafe {
                self.item.setTarget(Some(&target));
            }
            target
        });
    }

    fn clear_target(&mut self) {
        // SAFETY: Clearing the non-retained target before releasing our strong
        // reference ensures retained native items cannot message freed storage.
        unsafe {
            self.item.setTarget(None);
        }
        if let Some(target) = self.target.take() {
            target.ivars().callback.retire();
        }
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        self.clear_target();
        // SAFETY: An externally retained item must not resolve activate: through
        // the responder chain after its scoped target has been removed.
        unsafe {
            self.item.setAction(None);
        }
    }
}
