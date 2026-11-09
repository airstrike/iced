use std::collections::HashSet;
use std::rc::Rc;

use crate::core::menu;

use super::binding::Binding;
use super::callback::Callback;

pub struct Branch {
    pub title: String,
    pub enabled: bool,
    pub role: Option<menu::Role>,
    pub entries: Vec<Entry>,
}

pub enum Entry {
    Item(Item),
    Submenu(Branch),
    Separator,
}

pub struct Item {
    pub title: String,
    pub enabled: bool,
    pub checked: bool,
    pub shortcut: Option<menu::Shortcut>,
    pub icon: Option<menu::Icon>,
    pub callback: Option<Rc<Callback>>,
}

impl Branch {
    pub(super) fn claim_shortcuts(&mut self, claimed: &mut HashSet<menu::Shortcut>) {
        for entry in &mut self.entries {
            match entry {
                Entry::Item(item) => {
                    if !item.enabled
                        || item
                            .shortcut
                            .as_ref()
                            .is_some_and(|shortcut| !claimed.insert(shortcut.clone()))
                    {
                        item.shortcut = None;
                    }
                }
                Entry::Submenu(branch) => branch.claim_shortcuts(claimed),
                Entry::Separator => {}
            }
        }
    }

    pub(super) fn from_menu<Message: 'static>(
        menu: &menu::description::Menu,
        parent_enabled: bool,
        binding: &Rc<Binding<Message>>,
        path: &mut Vec<usize>,
    ) -> Self {
        let enabled = parent_enabled && menu.is_enabled();
        let title = menu.title().to_owned();
        let role = menu.native_role();
        let entries = menu
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                path.push(index);
                let entry = match entry {
                    menu::description::Entry::Submenu(menu) => {
                        Entry::Submenu(Self::from_menu(menu, enabled, binding, path))
                    }
                    menu::description::Entry::Separator => Entry::Separator,
                    menu::description::Entry::Item(item) => {
                        let enabled = enabled
                            && item.is_enabled()
                            && (!matches!(item.action(), Some(menu::description::Action::Edit(_)))
                                || binding.window().is_some());
                        let callback = enabled.then(|| binding.callback(path.clone()));
                        Entry::Item(Item {
                            title: item.title().to_owned(),
                            enabled,
                            checked: item.is_checked(),
                            shortcut: item.shortcut_key().cloned(),
                            icon: item.image().cloned(),
                            callback,
                        })
                    }
                };
                let _ = path.pop();
                entry
            })
            .collect();
        Self {
            title,
            enabled,
            role,
            entries,
        }
    }
}
