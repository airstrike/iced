use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSMenu, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSString};

use crate::core::menu;
use crate::menu::branch;

use super::delegate::Delegate;
use super::entry::Entry;

pub(super) struct Branch {
    pub(super) item: Retained<NSMenuItem>,
    pub(super) menu: Retained<NSMenu>,
    pub(super) role: Option<menu::Role>,
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
            role: None,
            entries: Vec::new(),
        };
        branch.update(snapshot, delegate);
        branch
    }

    pub(super) fn update(&mut self, snapshot: branch::Branch, delegate: &Delegate) {
        self.role = snapshot.role;
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
            for (index, entry) in snapshot.entries.into_iter().enumerate() {
                let entry = Entry::new(entry, delegate);
                // Window/Help registration may append system-owned entries.
                self.menu.insertItem_atIndex(entry.item(), index as isize);
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
