use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::NSMenuItem;
use objc2_foundation::MainThreadMarker;

use crate::menu::branch;

use super::branch::Branch;
use super::delegate::Delegate;
use super::item::Item;

pub(super) enum Entry {
    Item(Item),
    Submenu(Branch),
    Separator(Retained<NSMenuItem>),
}

impl Entry {
    pub(super) fn new(snapshot: branch::Entry, delegate: &Delegate) -> Self {
        let mtm = MainThreadMarker::from(delegate);
        match snapshot {
            branch::Entry::Item(snapshot) => {
                let mut item = Item {
                    item: NSMenuItem::new(mtm),
                    target: None,
                };
                // SAFETY: Every target installed by Item::update implements this
                // exact selector, and Item retains it until the target is cleared.
                unsafe {
                    item.item.setAction(Some(sel!(activate:)));
                }
                item.update(snapshot, delegate);
                Self::Item(item)
            }
            branch::Entry::Submenu(snapshot) => Self::Submenu(Branch::new(snapshot, delegate)),
            branch::Entry::Separator => Self::Separator(NSMenuItem::separatorItem(mtm)),
        }
    }

    pub(super) fn item(&self) -> &NSMenuItem {
        match self {
            Self::Item(item) => &item.item,
            Self::Submenu(branch) => &branch.item,
            Self::Separator(item) => item,
        }
    }
}
