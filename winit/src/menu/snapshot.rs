use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::{Arc, Weak};

use crate::Proxy;
use crate::core::{menu, window};
use crate::runtime::Action;

pub struct Snapshot {
    pub roots: Vec<Branch>,
}

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

/// An activation keeps its factory and captured window; retired targets are inert.
pub struct Callback {
    active: Cell<bool>,
    scope: Rc<Scope>,
    activate: Box<dyn Fn()>,
}

pub struct Scope {
    window: Option<window::Id>,
    is_current: Box<dyn Fn() -> bool>,
}

impl Scope {
    fn capture(
        window: Option<(window::Id, &Arc<winit::window::Window>)>,
        focus: crate::window::Focus,
    ) -> Self {
        match window {
            Some((id, raw)) => {
                let raw: Weak<winit::window::Window> = Arc::downgrade(raw);
                Self {
                    window: Some(id),
                    is_current: Box::new(move || {
                        focus.get() == Some(id)
                            && raw.upgrade().is_some_and(|window| window.has_focus())
                    }),
                }
            }
            None => Self {
                window: None,
                is_current: Box::new(|| true),
            },
        }
    }
}

impl Callback {
    pub fn new(scope: Rc<Scope>, activate: impl Fn() + 'static) -> Self {
        Self {
            active: Cell::new(true),
            scope,
            activate: Box::new(activate),
        }
    }

    pub fn invoke(&self) {
        if self.active.get() && (self.scope.is_current)() {
            (self.activate)();
        }
    }

    pub fn retire(&self) {
        self.active.set(false);
    }
}

impl Snapshot {
    pub fn claim_shortcuts(&mut self, inherited: &mut HashSet<menu::Shortcut>) {
        for root in &mut self.roots {
            root.claim_shortcuts(inherited);
        }
    }

    pub fn from_bar<Message: 'static>(
        bar: menu::Bar<Message>,
        window: Option<(window::Id, &Arc<winit::window::Window>)>,
        proxy: &Proxy<Message>,
        focus: crate::window::Focus,
    ) -> Self {
        let scope = Rc::new(Scope::capture(window, focus));
        let proxy = proxy.clone();
        let send: Rc<dyn Fn(Action<Message>)> = Rc::new(move |action| proxy.send_action(action));
        Self {
            roots: bar
                .menus()
                .iter()
                .map(|menu| Branch::from_menu(menu, true, &scope, &send))
                .collect(),
        }
    }
}

impl Branch {
    fn claim_shortcuts(&mut self, claimed: &mut HashSet<menu::Shortcut>) {
        for entry in &mut self.entries {
            match entry {
                Entry::Item(item) => {
                    if !item.enabled
                        || item
                            .shortcut
                            .is_some_and(|shortcut| !claimed.insert(shortcut))
                    {
                        item.shortcut = None;
                    }
                }
                Entry::Submenu(branch) => branch.claim_shortcuts(claimed),
                Entry::Separator => {}
            }
        }
    }

    fn from_menu<Message: 'static>(
        menu: &menu::Menu<Message>,
        parent_enabled: bool,
        scope: &Rc<Scope>,
        send: &Rc<dyn Fn(Action<Message>)>,
    ) -> Self {
        let window = scope.window;
        let enabled = parent_enabled && menu.is_enabled();
        let entries = menu
            .entries()
            .iter()
            .map(|entry| match entry {
                menu::Entry::Submenu(menu) => {
                    Entry::Submenu(Self::from_menu(menu, enabled, scope, send))
                }
                menu::Entry::Separator => Entry::Separator,
                menu::Entry::Item(item) => {
                    let action = item.action().cloned();
                    let enabled = enabled
                        && item.is_enabled()
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
                        title: item.title().to_owned(),
                        enabled,
                        checked: item.is_checked(),
                        shortcut: item.shortcut_key(),
                        callback,
                    })
                }
            })
            .collect();
        Self {
            title: menu.title().to_owned(),
            enabled,
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Callback, Scope};
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn retiring_a_target_never_reinterprets_its_factory() {
        let value = Rc::new(Cell::new(0));
        let old = Rc::new(Callback::new(
            Rc::new(Scope::capture(None, crate::window::Focus::default())),
            {
                let value = value.clone();
                move || value.set(1)
            },
        ));
        let retained = old.clone();
        old.invoke();
        assert_eq!(value.get(), 1);
        old.retire();
        let replacement = Callback::new(
            Rc::new(Scope::capture(None, crate::window::Focus::default())),
            {
                let value = value.clone();
                move || value.set(2)
            },
        );
        replacement.invoke();
        retained.invoke();
        assert_eq!(value.get(), 2);
    }

    #[test]
    fn releasing_target_releases_its_captured_state() {
        let state = Rc::new(());
        let weak = Rc::downgrade(&state);
        let callback = Callback::new(
            Rc::new(Scope::capture(None, crate::window::Focus::default())),
            move || {
                let _ = &state;
            },
        );
        assert!(weak.upgrade().is_some());
        drop(callback);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn a_frozen_window_callback_rechecks_current_focus_before_running_its_factory() {
        let key = Rc::new(Cell::new(true));
        let invocations = Rc::new(Cell::new(0));
        let scope = Rc::new(Scope {
            window: Some(crate::core::window::Id::unique()),
            is_current: Box::new({
                let key = key.clone();
                move || key.get()
            }),
        });
        let callback = Callback::new(scope, {
            let invocations = invocations.clone();
            move || invocations.set(invocations.get() + 1)
        });
        callback.invoke();
        key.set(false);
        callback.invoke();
        assert_eq!(invocations.get(), 1);
    }

    #[test]
    fn identical_presentation_does_not_cache_an_old_factory() {
        use crate::core::menu;
        use crate::runtime::Action;
        use std::cell::RefCell;

        let output = Rc::new(RefCell::new(Vec::new()));
        let send: Rc<dyn Fn(Action<usize>)> = Rc::new({
            let output = output.clone();
            move |action| {
                if let Action::Output(value) = action {
                    output.borrow_mut().push(value);
                }
            }
        });
        let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
        let make = |value| {
            super::Branch::from_menu(
                &menu::submenu("Same").push(menu::item("Same").on_press_with(move || value)),
                true,
                &scope,
                &send,
            )
        };
        let first = make(1);
        let second = make(2);
        assert_eq!(first.title, second.title);
        let (super::Entry::Item(first), super::Entry::Item(second)) =
            (&first.entries[0], &second.entries[0])
        else {
            panic!("the test declares item entries");
        };
        assert_eq!(first.title, second.title);
        first.callback.as_ref().unwrap().invoke();
        first.callback.as_ref().unwrap().retire();
        second.callback.as_ref().unwrap().invoke();
        first.callback.as_ref().unwrap().invoke();
        assert_eq!(&*output.borrow(), &[1, 2]);
    }

    #[test]
    fn inherited_then_first_enabled_shortcuts_win_and_disabled_ancestors_are_inert() {
        use crate::core::{keyboard::Modifiers, menu};
        use crate::runtime::Action;
        use std::collections::HashSet;

        let key = menu::Shortcut::new(Modifiers::LOGO, 'k').unwrap();
        let inherited = menu::Shortcut::new(Modifiers::LOGO, 'q').unwrap();
        let send: Rc<dyn Fn(Action<()>)> = Rc::new(|_| panic!("disabled factories must not run"));
        let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
        let menu = menu::submenu("Root")
            .push(
                menu::submenu("Disabled").enabled(false).push(
                    menu::item("Hidden accelerator")
                        .on_press(())
                        .shortcut(Modifiers::LOGO, 'k'),
                ),
            )
            .push(
                menu::item("First")
                    .on_press(())
                    .shortcut(Modifiers::LOGO, 'k'),
            )
            .push(
                menu::item("Duplicate")
                    .on_press(())
                    .shortcut(Modifiers::LOGO, 'k'),
            )
            .push(
                menu::item("Inherited")
                    .on_press(())
                    .shortcut(Modifiers::LOGO, 'q'),
            );
        let mut branch = super::Branch::from_menu(&menu, true, &scope, &send);
        branch.claim_shortcuts(&mut HashSet::from([inherited]));
        let super::Entry::Submenu(disabled) = &branch.entries[0] else {
            panic!("the first entry is a submenu");
        };
        let super::Entry::Item(disabled) = &disabled.entries[0] else {
            panic!("the disabled submenu contains an item");
        };
        assert!(!disabled.enabled);
        assert!(disabled.callback.is_none());
        assert!(disabled.shortcut.is_none());
        for (entry, expected) in branch.entries[1..].iter().zip([Some(key), None, None]) {
            let super::Entry::Item(item) = entry else {
                panic!("the remaining entries are items");
            };
            assert_eq!(item.shortcut, expected);
        }
    }

    #[test]
    fn native_edit_keeps_the_captured_window_and_is_disabled_without_one() {
        use crate::core::{edit, menu, window};
        use crate::runtime::Action;
        use std::cell::RefCell;

        let output = Rc::new(RefCell::new(Vec::new()));
        let send: Rc<dyn Fn(Action<()>)> = Rc::new({
            let output = output.clone();
            move |action| {
                if let Action::Event { window, event } = action {
                    output.borrow_mut().push((window, event));
                }
            }
        });
        let window = window::Id::unique();
        let menu = menu::submenu("Edit").push(menu::Item::edit(edit::Command::Paste));
        let scope = Rc::new(Scope {
            window: Some(window),
            is_current: Box::new(|| true),
        });
        let branch = super::Branch::from_menu(&menu, true, &scope, &send);
        let super::Entry::Item(item) = &branch.entries[0] else {
            panic!("the Edit menu contains an item");
        };
        item.callback.as_ref().unwrap().invoke();
        assert!(
            matches!(&output.borrow()[..], [(id, crate::core::Event::Edit(edit::Command::Paste))] if *id == window)
        );

        let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
        let branch = super::Branch::from_menu(&menu, true, &scope, &send);
        let super::Entry::Item(item) = &branch.entries[0] else {
            panic!("the Edit menu contains an item");
        };
        assert!(!item.enabled);
        assert!(item.callback.is_none());
    }
}
