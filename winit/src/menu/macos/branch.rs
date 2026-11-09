use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSMenu, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSString};

use crate::menu::branch;

use super::delegate::Delegate;
use super::entry::Entry;

pub(super) struct Branch {
    pub(super) item: Retained<NSMenuItem>,
    menu: Retained<NSMenu>,
    entries: Vec<Entry>,
}

impl Branch {
    pub(super) fn new(snapshot: branch::Branch, delegate: &Delegate) -> Self {
        let mtm = MainThreadMarker::from(delegate);
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        menu.setDelegate(Some(ProtocolObject::from_ref(delegate)));
        let item = NSMenuItem::new(mtm);
        item.setSubmenu(Some(&menu));
        let mut branch = Self {
            item,
            menu,
            entries: Vec::new(),
        };
        branch.update(snapshot, delegate);
        branch
    }

    pub(super) fn update(&mut self, snapshot: branch::Branch, delegate: &Delegate) {
        let title = NSString::from_str(&snapshot.title);
        self.item.setTitle(&title);
        self.item.setEnabled(snapshot.enabled);
        self.menu.setTitle(&title);

        let same_shape = self.entries.len() == snapshot.entries.len()
            && self
                .entries
                .iter()
                .zip(&snapshot.entries)
                .all(|(entry, snapshot)| {
                    matches!(
                        (entry, snapshot),
                        (Entry::Item(_), branch::Entry::Item(_))
                            | (Entry::Submenu(_), branch::Entry::Submenu(_))
                            | (Entry::Separator(_), branch::Entry::Separator)
                    )
                });

        if same_shape {
            for (entry, snapshot) in self.entries.iter_mut().zip(snapshot.entries) {
                match (entry, snapshot) {
                    (Entry::Item(item), branch::Entry::Item(snapshot)) => {
                        item.update(snapshot, delegate);
                    }
                    (Entry::Submenu(branch), branch::Entry::Submenu(snapshot)) => {
                        branch.update(snapshot, delegate);
                    }
                    _ => {}
                }
            }
        } else {
            for entry in self.entries.drain(..) {
                self.menu.removeItem(entry.item());
            }
            for entry in snapshot.entries {
                let entry = Entry::new(entry, delegate);
                self.menu.addItem(entry.item());
                self.entries.push(entry);
            }
        }
    }
}

impl Drop for Branch {
    fn drop(&mut self) {
        self.menu.setDelegate(None);
    }
}
