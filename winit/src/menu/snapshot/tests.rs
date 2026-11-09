use crate::menu::callback::Callback;
use crate::menu::scope::Scope;
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
    let (crate::menu::branch::Entry::Item(first), crate::menu::branch::Entry::Item(second)) =
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
    let crate::menu::branch::Entry::Submenu(disabled) = &branch.entries[0] else {
        panic!("the first entry is a submenu");
    };
    let crate::menu::branch::Entry::Item(disabled) = &disabled.entries[0] else {
        panic!("the disabled submenu contains an item");
    };
    assert!(!disabled.enabled);
    assert!(disabled.callback.is_none());
    assert!(disabled.shortcut.is_none());
    for (entry, expected) in branch.entries[1..].iter().zip([Some(key), None, None]) {
        let crate::menu::branch::Entry::Item(item) = entry else {
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
    let crate::menu::branch::Entry::Item(item) = &branch.entries[0] else {
        panic!("the Edit menu contains an item");
    };
    item.callback.as_ref().unwrap().invoke();
    assert!(
        matches!(&output.borrow()[..], [(id, crate::core::Event::Edit(edit::Command::Paste))] if *id == window)
    );

    let scope = Rc::new(Scope::capture(None, crate::window::Focus::default()));
    let branch = super::Branch::from_menu(&menu, true, &scope, &send);
    let crate::menu::branch::Entry::Item(item) = &branch.entries[0] else {
        panic!("the Edit menu contains an item");
    };
    assert!(!item.enabled);
    assert!(item.callback.is_none());
}
