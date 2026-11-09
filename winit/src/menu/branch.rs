use std::collections::HashSet;
use std::rc::Rc;

use crate::core::menu;
use crate::runtime::Action;

use super::callback::Callback;
use super::scope::Scope;

pub struct Branch {
    pub title: String,
    pub enabled: bool,
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
        menu: menu::Menu<Message>,
        parent_enabled: bool,
        scope: &Rc<Scope>,
        send: &Rc<dyn Fn(Action<Message>)>,
    ) -> Self {
        let window = scope.window;
        let enabled = parent_enabled && menu.is_enabled();
        let title = menu.title().to_owned();
        let entries = menu
            .into_iter()
            .map(|entry| match entry {
                menu::Entry::Submenu(menu) => {
                    Entry::Submenu(Self::from_menu(menu, enabled, scope, send))
                }
                menu::Entry::Separator => Entry::Separator,
                menu::Entry::Item(item) => {
                    let enabled = enabled && item.is_enabled();
                    let title = item.title().to_owned();
                    let checked = item.is_checked();
                    let shortcut = item.shortcut_key().cloned();
                    let action = item.into_action();
                    let enabled = enabled
                        && (!matches!(action, Some(menu::Action::Edit(_))) || window.is_some());
                    let callback = action.filter(|_| enabled).map(|action| {
                        let send = send.clone();
                        Rc::new(Callback::new(scope.clone(), move || match &action {
                            menu::Action::Message(factory) => send(Action::Output(factory())),
                            menu::Action::Edit(command) => {
                                if let Some(window) = window {
                                    send(Action::Event {
                                        window,
                                        event: crate::core::Event::Edit(*command),
                                    });
                                }
                            }
                        }))
                    });
                    Entry::Item(Item {
                        title,
                        enabled,
                        checked,
                        shortcut,
                        callback,
                    })
                }
            })
            .collect();
        Self {
            title,
            enabled,
            entries,
        }
    }
}
